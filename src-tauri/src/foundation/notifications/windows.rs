//! User-scoped, unpackaged Win32 ToastGeneric notifications with a COM activator.
//! All WinRT objects live on one MTA thread. No PowerShell identity or installer.
use super::*;
use crate::foundation::AppError;
use ::windows::Data::Xml::Dom::XmlDocument;
use ::windows::UI::Notifications::{
    NotificationSetting, ToastNotification, ToastNotificationManager, ToastNotifier,
};
use ::windows::Win32::Foundation::{CLASS_E_NOAGGREGATION, E_INVALIDARG, E_POINTER, PROPERTYKEY};
use ::windows::Win32::Globalization::GetUserDefaultUILanguage;
use ::windows::Win32::System::Com::StructuredStorage::{InitPropVariantFromCLSID, PROPVARIANT};
use ::windows::Win32::System::Com::*;
use ::windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize};
use ::windows::Win32::UI::Notifications::*;
use ::windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
use ::windows::Win32::UI::Shell::{
    FOLDERID_Programs, IShellLinkW, KF_FLAG_DEFAULT, SHGetKnownFolderPath,
    SetCurrentProcessExplicitAppUserModelID, ShellLink,
};
use ::windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
use ::windows::core::{BOOL, GUID, HSTRING, IUnknown, Interface, PCWSTR, Ref, implement};
use parking_lot::Mutex;
use sha2::{Digest, Sha256};
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::time::Duration;
use winreg::{RegKey, enums::HKEY_CURRENT_USER};

const APP_ID_KEY: PROPERTYKEY = PROPERTYKEY {
    fmtid: GUID::from_u128(0x9f4c2855_9f79_4b39_a8d0_e1d42de1d5f3),
    pid: 5,
};
const ACTIVATOR_KEY: PROPERTYKEY = PROPERTYKEY {
    fmtid: APP_ID_KEY.fmtid,
    pid: 26,
};

enum Command {
    Show(ToastMessage, Sender<AppResult<bool>>),
    Remove(String, Sender<AppResult<()>>),
    Clear(Sender<AppResult<()>>),
    Status(Sender<PlatformNotificationStatus>),
    Stop(Sender<()>),
}

pub struct SystemNotifications {
    sender: Sender<Command>,
    status: Arc<Mutex<PlatformNotificationStatus>>,
}

impl SystemNotifications {
    pub fn new(paths: &AppPaths, handler: ActivationHandler) -> Self {
        let test = cfg!(debug_assertions) || std::env::var("ABYA_TEST_MODE").as_deref() == Ok("1");
        let digest = format!(
            "{:x}",
            Sha256::digest(paths.data_dir.to_string_lossy().to_lowercase().as_bytes())
        );
        let identity = if test {
            format!("com.abya.desktop-development-tool.test.{}", &digest[..16])
        } else {
            "com.abya.desktop-development-tool".into()
        };
        let status = Arc::new(Mutex::new(PlatformNotificationStatus {
            available: false,
            reason: "initializing".into(),
            identity: identity.clone(),
            test_environment: test,
        }));
        let (sender, receiver) = mpsc::channel();
        let shared_status = status.clone();
        let directory = paths.data_dir.join("Notifications");
        std::thread::spawn(move || {
            let initialized = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
            let uninitialize = initialized.is_ok();
            let native =
                initialized.and_then(|_| Native::new(&identity, &directory, test, handler));
            match native {
                Ok(native) => {
                    *shared_status.lock() = native.status();
                    while let Ok(command) = receiver.recv() {
                        match command {
                            Command::Show(message, reply) => {
                                let result = native.show(&message).map_err(platform_error);
                                if let Err(error) = &result {
                                    shared_status.lock().reason = error.message.clone();
                                }
                                let _ = reply.send(result);
                            }
                            Command::Remove(key, reply) => {
                                let _ = reply.send(native.remove(&key).map_err(platform_error));
                            }
                            Command::Clear(reply) => {
                                let _ = reply.send(native.clear().map_err(platform_error));
                            }
                            Command::Status(reply) => {
                                let next = native.status();
                                *shared_status.lock() = next.clone();
                                let _ = reply.send(next);
                            }
                            Command::Stop(reply) => {
                                let _ = native.clear();
                                drop(native);
                                let _ = reply.send(());
                                break;
                            }
                        }
                    }
                }
                Err(error) => {
                    shared_status.lock().reason = platform_error(error).message;
                    // Dropping the receiver makes all pending callers fail promptly.
                }
            }
            if uninitialize {
                unsafe {
                    RoUninitialize();
                }
            }
        });
        Self { sender, status }
    }

    fn call<T>(&self, command: impl FnOnce(Sender<AppResult<T>>) -> Command) -> AppResult<T> {
        let (sender, receiver) = mpsc::channel();
        self.sender
            .send(command(sender))
            .map_err(|_| AppError::validation("notificationUnavailable"))?;
        receiver
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| AppError::validation("notificationTimeout"))?
    }
}

impl NotificationBackend for SystemNotifications {
    fn status(&self) -> PlatformNotificationStatus {
        let (sender, receiver) = mpsc::channel();
        if self.sender.send(Command::Status(sender)).is_ok()
            && let Ok(status) = receiver.recv_timeout(Duration::from_secs(2))
        {
            return status;
        }
        self.status.lock().clone()
    }
    fn show(&self, message: ToastMessage) -> AppResult<bool> {
        self.call(|reply| Command::Show(message, reply))
    }
    fn remove(&self, key: &str) -> AppResult<()> {
        self.call(|reply| Command::Remove(key.into(), reply))
    }
    fn clear(&self) -> AppResult<()> {
        self.call(Command::Clear)
    }
    fn shutdown(&self) {
        let (sender, receiver) = mpsc::channel();
        if self.sender.send(Command::Stop(sender)).is_ok() {
            let _ = receiver.recv_timeout(Duration::from_secs(3));
        }
    }
}

struct Native {
    identity: String,
    notifier: ToastNotifier,
    cookie: u32,
    test: bool,
    _registration: Registration,
}

// Scope partial initialization too: an error after creating HKCU keys or the
// shortcut must not leave a test identity pointing at a disposable executable.
struct Registration {
    identity: String,
    clsid: String,
    shortcut: Option<PathBuf>,
    test: bool,
}
impl Drop for Registration {
    fn drop(&mut self) {
        if self.test {
            let key = RegKey::predef(HKEY_CURRENT_USER);
            let _ = key.delete_subkey_all(format!(
                r"Software\Classes\AppUserModelId\{}",
                self.identity
            ));
            let _ = key.delete_subkey_all(format!(r"Software\Classes\CLSID\{}", self.clsid));
            if let Some(shortcut) = &self.shortcut {
                let _ = std::fs::remove_file(shortcut);
            }
        }
    }
}

impl Native {
    fn new(
        identity: &str,
        directory: &Path,
        test: bool,
        handler: ActivationHandler,
    ) -> ::windows::core::Result<Self> {
        let io = |e: std::io::Error| {
            ::windows::core::Error::from_hresult(::windows::core::HRESULT::from_win32(
                e.raw_os_error().unwrap_or(1) as u32,
            ))
        };
        let exe = std::env::current_exe().map_err(io)?;
        std::fs::create_dir_all(directory).map_err(io)?;
        let icon = directory.join("app.ico");
        let icon_bytes = include_bytes!("../../../icons/icon.ico");
        if std::fs::read(&icon).ok().as_deref() != Some(icon_bytes.as_slice()) {
            std::fs::write(&icon, icon_bytes).map_err(io)?;
        }
        let hash = Sha256::digest(identity.as_bytes());
        let guid = GUID::from_u128(u128::from_be_bytes(hash[..16].try_into().unwrap()));
        let clsid = format!("{{{:?}}}", guid);
        let mut registration = Registration {
            identity: identity.into(),
            clsid: clsid.clone(),
            shortcut: None,
            test,
        };
        let name = if test {
            "ABYA Development Tool (测试通知)"
        } else {
            "ABYA Development Tool"
        };
        let key = RegKey::predef(HKEY_CURRENT_USER);
        let (app_key, _) = key
            .create_subkey(format!(r"Software\Classes\AppUserModelId\{identity}"))
            .map_err(io)?;
        app_key.set_value("DisplayName", &name).map_err(io)?;
        app_key
            .set_value("IconUri", &icon.to_string_lossy().as_ref())
            .map_err(io)?;
        app_key.set_value("CustomActivator", &clsid).map_err(io)?;
        let (server, _) = key
            .create_subkey(format!(r"Software\Classes\CLSID\{clsid}\LocalServer32"))
            .map_err(io)?;
        // A stale test notification must never launch a process on the real user database.
        let argument = if test {
            "--abya-toast-test-only"
        } else {
            "--abya-toast-activated"
        };
        server
            .set_value("", &format!("\"{}\" {argument}", exe.display()))
            .map_err(io)?;
        let shortcut;
        unsafe {
            SetCurrentProcessExplicitAppUserModelID(&HSTRING::from(identity))?;
            let programs = SHGetKnownFolderPath(&FOLDERID_Programs, KF_FLAG_DEFAULT, None)?;
            let path = PathBuf::from(programs.to_string()?);
            CoTaskMemFree(Some(programs.0.cast()));
            let suffix = if test {
                format!(" {}", &identity[identity.len() - 16..])
            } else {
                String::new()
            };
            shortcut = path.join(format!("{name}{suffix}.lnk"));
            registration.shortcut = Some(shortcut.clone());
            let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
            link.SetPath(&HSTRING::from(exe.to_string_lossy().as_ref()))?;
            link.SetWorkingDirectory(&HSTRING::from(
                exe.parent().unwrap_or(directory).to_string_lossy().as_ref(),
            ))?;
            if test {
                link.SetArguments(&HSTRING::from("--abya-toast-test-only"))?;
            }
            link.SetDescription(&HSTRING::from(name))?;
            link.SetIconLocation(&HSTRING::from(icon.to_string_lossy().as_ref()), 0)?;
            let store: IPropertyStore = link.cast()?;
            store.SetValue(&APP_ID_KEY, &PROPVARIANT::from(identity))?;
            store.SetValue(&ACTIVATOR_KEY, &InitPropVariantFromCLSID(&guid)?)?;
            store.Commit()?;
            let persist: IPersistFile = link.cast()?;
            persist.Save(&HSTRING::from(shortcut.to_string_lossy().as_ref()), true)?;
        }
        let factory: IClassFactory = ToastFactory {
            application_id: identity.into(),
            handler,
        }
        .into();
        let cookie = unsafe {
            CoRegisterClassObject(&guid, &factory, CLSCTX_LOCAL_SERVER, REGCLS_MULTIPLEUSE)?
        };
        let notifier =
            match ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(identity)) {
                Ok(value) => value,
                Err(error) => {
                    unsafe {
                        let _ = CoRevokeClassObject(cookie);
                    }
                    return Err(error);
                }
            };
        let native = Self {
            identity: identity.into(),
            notifier,
            cookie,
            test,
            _registration: registration,
        };
        native.preregister()?;
        Ok(native)
    }

    fn preregister(&self) -> ::windows::core::Result<()> {
        // Fresh unpackaged AUMIDs have no OS settings entry until their first
        // notification. Match the Windows Community Toolkit bootstrap: suppressed,
        // silent, short expiry, removed immediately. Actual task toasts use defaults.
        if !self
            .notifier
            .Setting()
            .is_err_and(|e| e.code().0 as u32 == 0x80070490)
        {
            return Ok(());
        }
        let xml = XmlDocument::new()?;
        xml.LoadXml(&HSTRING::from("<toast launch=\"test\"><visual><binding template=\"ToastGeneric\"><text>ABYA</text></binding></visual><audio silent=\"true\"/></toast>"))?;
        let toast = ToastNotification::CreateToastNotification(&xml)?;
        toast.SetSuppressPopup(true)?;
        toast.SetTag(&HSTRING::from("registration"))?;
        toast.SetGroup(&HSTRING::from("registration"))?;
        let expiry = ::windows::Foundation::PropertyValue::CreateDateTime(
            ::windows::Foundation::DateTime {
                UniversalTime: (chrono::Utc::now().timestamp_millis() + 15_000) * 10_000
                    + 116_444_736_000_000_000,
            },
        )?
        .cast::<::windows::Foundation::IReference<::windows::Foundation::DateTime>>()?;
        toast.SetExpirationTime(&expiry)?;
        self.notifier.Show(&toast)?;
        ToastNotificationManager::History()?.RemoveGroupedTagWithId(
            &HSTRING::from("registration"),
            &HSTRING::from("registration"),
            &HSTRING::from(&self.identity),
        )?;
        Ok(())
    }

    fn status(&self) -> PlatformNotificationStatus {
        let reason = match self.notifier.Setting() {
            Ok(NotificationSetting::Enabled) => "",
            Ok(NotificationSetting::DisabledForApplication) => "disabledForApplication",
            Ok(NotificationSetting::DisabledForUser) => "disabledForUser",
            Ok(NotificationSetting::DisabledByGroupPolicy) => "disabledByGroupPolicy",
            Ok(NotificationSetting::DisabledByManifest) => "disabledByManifest",
            Ok(_) => "unavailable",
            Err(error) => {
                return PlatformNotificationStatus {
                    available: false,
                    reason: platform_error(error).message,
                    identity: self.identity.clone(),
                    test_environment: self.test,
                };
            }
        };
        PlatformNotificationStatus {
            available: reason.is_empty(),
            reason: reason.into(),
            identity: self.identity.clone(),
            test_environment: self.test,
        }
    }
    fn show(&self, message: &ToastMessage) -> ::windows::core::Result<bool> {
        if !valid_key(&message.key) {
            return Err(E_INVALIDARG.into());
        }
        if !message.test && is_foreground() {
            return Ok(false);
        }
        let xml = XmlDocument::new()?;
        xml.LoadXml(&HSTRING::from(toast_xml(message)))?;
        let toast = ToastNotification::CreateToastNotification(&xml)?;
        let (tag, group) = tag_group(&message.key);
        toast.SetTag(&HSTRING::from(tag))?;
        toast.SetGroup(&HSTRING::from(group))?;
        self.notifier.Show(&toast)?;
        Ok(true)
    }
    fn remove(&self, key: &str) -> ::windows::core::Result<()> {
        if !valid_key(key) {
            return Err(E_INVALIDARG.into());
        }
        let (tag, group) = tag_group(key);
        ToastNotificationManager::History()?.RemoveGroupedTagWithId(
            &HSTRING::from(tag),
            &HSTRING::from(group),
            &HSTRING::from(&self.identity),
        )
    }
    fn clear(&self) -> ::windows::core::Result<()> {
        ToastNotificationManager::History()?.ClearWithId(&HSTRING::from(&self.identity))
    }
}

impl Drop for Native {
    fn drop(&mut self) {
        unsafe {
            let _ = CoRevokeClassObject(self.cookie);
        }
    }
}

fn tag_group(key: &str) -> (&str, &str) {
    if key == "test" {
        ("test", "abya-test")
    } else {
        (&key[..16], &key[16..32])
    }
}
fn platform_error(error: ::windows::core::Error) -> AppError {
    AppError::new(
        "notificationUnavailable",
        format!("Windows 通知不可用（0x{:08X}）", error.code().0 as u32),
        "",
    )
}
pub fn is_foreground() -> bool {
    unsafe {
        let window = GetForegroundWindow();
        let mut pid = 0;
        GetWindowThreadProcessId(window, Some(&mut pid));
        pid == std::process::id()
    }
}
pub fn system_english() -> bool {
    unsafe { GetUserDefaultUILanguage() & 0x3ff != 0x04 }
}

#[implement(INotificationActivationCallback)]
struct ToastActivator {
    application_id: String,
    handler: ActivationHandler,
}

impl INotificationActivationCallback_Impl for ToastActivator_Impl {
    fn Activate(
        &self,
        app: &PCWSTR,
        args: &PCWSTR,
        _: *const NOTIFICATION_USER_INPUT_DATA,
        _: u32,
    ) -> ::windows::core::Result<()> {
        if app.is_null() || args.is_null() {
            return Err(E_INVALIDARG.into());
        }
        // Windows supplies null-terminated strings. Bound traversal before decoding.
        let read = |ptr: PCWSTR, maximum: usize| -> ::windows::core::Result<String> {
            for length in 0..=maximum {
                if unsafe { *ptr.0.add(length) } == 0 {
                    return String::from_utf16(unsafe {
                        std::slice::from_raw_parts(ptr.0, length)
                    })
                    .map_err(|_| E_INVALIDARG.into());
                }
            }
            Err(E_INVALIDARG.into())
        };
        let app = read(*app, 128)?;
        let key = read(*args, 64)?;
        if app != self.application_id || !valid_key(&key) {
            return Err(E_INVALIDARG.into());
        }
        (self.handler)(key);
        Ok(())
    }
}

#[implement(IClassFactory)]
struct ToastFactory {
    application_id: String,
    handler: ActivationHandler,
}

impl IClassFactory_Impl for ToastFactory_Impl {
    fn CreateInstance(
        &self,
        outer: Ref<IUnknown>,
        iid: *const GUID,
        result: *mut *mut c_void,
    ) -> ::windows::core::Result<()> {
        if result.is_null() || iid.is_null() {
            return Err(E_POINTER.into());
        }
        unsafe {
            *result = std::ptr::null_mut();
        }
        if outer.is_some() {
            return Err(CLASS_E_NOAGGREGATION.into());
        }
        let activator: INotificationActivationCallback = ToastActivator {
            application_id: self.application_id.clone(),
            handler: self.handler.clone(),
        }
        .into();
        unsafe { activator.query(iid, result).ok() }
    }
    fn LockServer(&self, _: BOOL) -> ::windows::core::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "Targets only a marker-owned running portable notification fixture"]
    fn isolated_portable_notification_activation() {
        let root = PathBuf::from(
            std::env::var("ABYA_TEST_NOTIFICATION_FIXTURE").expect("fixture root required"),
        );
        let marker: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join("notification-fixture.json")).unwrap())
                .unwrap();
        assert_eq!(marker["schema"], "abya-notification-fixture/v1");
        let data = root
            .join("Local")
            .join("ABYA Desktop Development Tool")
            .join("Data");
        let digest = format!(
            "{:x}",
            Sha256::digest(data.to_string_lossy().to_lowercase().as_bytes())
        );
        let identity = format!("com.abya.desktop-development-tool.test.{}", &digest[..16]);
        unsafe {
            RoInitialize(RO_INIT_MULTITHREADED).unwrap();
        }
        {
            let history = ToastNotificationManager::History()
                .unwrap()
                .GetHistoryWithId(&HSTRING::from(&identity))
                .unwrap();
            let mut toasts = Vec::new();
            for index in 0..history.Size().unwrap() {
                let toast = history.GetAt(index).unwrap();
                toasts.push(serde_json::json!({"tag":toast.Tag().unwrap().to_string(),"xml":toast.Content().unwrap().GetXml().unwrap().to_string()}));
            }
            println!(
                "{}",
                serde_json::json!({"identity":identity,"history":toasts})
            );
            if let Ok(key) = std::env::var("ABYA_TEST_NOTIFICATION_KEY") {
                assert!(valid_key(&key));
                let hash = Sha256::digest(identity.as_bytes());
                let guid = GUID::from_u128(u128::from_be_bytes(hash[..16].try_into().unwrap()));
                let callback: INotificationActivationCallback =
                    unsafe { CoCreateInstance(&guid, None, CLSCTX_LOCAL_SERVER).unwrap() };
                unsafe {
                    callback
                        .Activate(&HSTRING::from(&identity), &HSTRING::from(key), &[])
                        .unwrap();
                }
                println!("COM activation delivered to the isolated portable process");
            }
        }
        unsafe {
            RoUninitialize();
        }
    }

    #[test]
    #[ignore = "Displays a real isolated Windows notification; requires ABYA_TEST_WINDOWS_TOAST=1"]
    fn isolated_windows_toast_registration_history_and_com() {
        assert_eq!(std::env::var("ABYA_TEST_WINDOWS_TOAST").as_deref(), Ok("1"));
        let root = std::env::temp_dir().join(format!("abya-toast-{}", uuid::Uuid::new_v4()));
        let paths = AppPaths {
            data_dir: root.clone(),
            database_path: root.join("app.db"),
            bootstrap_logs_dir: root.join("logs"),
            reports_dir: root.join("reports"),
            archive_transfers_dir: root.join("transfers"),
            default_archive_root: root.join("archives"),
            default_workspace_root: root.join("workspaces"),
        };
        let (sender, receiver) = mpsc::channel();
        let backend = SystemNotifications::new(
            &paths,
            Arc::new(move |key| {
                let _ = sender.send(key);
            }),
        );
        let status = backend.status();
        println!("{}", serde_json::to_string(&status).unwrap());
        assert!(status.available, "{}", status.reason);
        assert!(status.test_environment);
        backend
            .show(ToastMessage {
                key: "test".into(),
                title: "ABYA：原生通知验收".into(),
                body: "独立测试身份；无任务操作。".into(),
                test: true,
            })
            .unwrap();
        unsafe {
            RoInitialize(RO_INIT_MULTITHREADED).unwrap();
        }
        let history = ToastNotificationManager::History()
            .unwrap()
            .GetHistoryWithId(&HSTRING::from(&status.identity))
            .unwrap();
        assert!(history.Size().unwrap() > 0);
        let hash = Sha256::digest(status.identity.as_bytes());
        let guid = GUID::from_u128(u128::from_be_bytes(hash[..16].try_into().unwrap()));
        let activator: INotificationActivationCallback =
            unsafe { CoCreateInstance(&guid, None, CLSCTX_LOCAL_SERVER).unwrap() };
        unsafe {
            activator
                .Activate(
                    &HSTRING::from(&status.identity),
                    &HSTRING::from("test"),
                    &[],
                )
                .unwrap();
        }
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(5)).unwrap(),
            "test"
        );
        drop(activator);
        drop(history);
        backend.remove("test").unwrap();
        assert_eq!(
            ToastNotificationManager::History()
                .unwrap()
                .GetHistoryWithId(&HSTRING::from(&status.identity))
                .unwrap()
                .Size()
                .unwrap(),
            0
        );
        backend.shutdown();
        assert!(
            RegKey::predef(HKEY_CURRENT_USER)
                .open_subkey(format!(
                    r"Software\Classes\AppUserModelId\{}",
                    status.identity
                ))
                .is_err()
        );
        unsafe {
            RoUninitialize();
        }
        let _ = std::fs::remove_dir_all(root);
    }
}
