-- ABYA pre-start countdown reference core v1.0.
-- Pure Lua: no engine calls. now_ms/C must use the same authority clock.
local M = {}
local FRAMES = {
  [3] = { asset = "assets/countdown-3.png", x = 433.666666666667, y = 223.083333333333, width = 326.666666666667, height = 505.833333333333 },
  [2] = { asset = "assets/countdown-2.png", x = 425.75, y = 226.833333333333, width = 342.5, height = 498.333333333333 },
  [1] = { asset = "assets/countdown-1.png", x = 475.333333333333, y = 225.583333333333, width = 243.333333333333, height = 500.833333333333 }
}
local function finite(n) return type(n) == "number" and n == n and n > -math.huge and n < math.huge end
function M.new(match_id, countdown_at_ms)
  assert(type(match_id) == "string" and #match_id > 0, "match_id required")
  assert(finite(countdown_at_ms), "finite authority C required")
  return { match_id = match_id, countdown_at_ms = countdown_at_ms,
    start_at_ms = countdown_at_ms + 3000, end_at_ms = countdown_at_ms + 23000,
    last_now_ms = -math.huge, last_phase = nil, cancelled = false }
end
function M.cancel(state) state.cancelled = true end
function M.sample(state, now_ms)
  assert(finite(now_ms), "finite authority now required")
  -- Prevent a corrected/local clock from replaying a completed phase.
  now_ms = math.max(now_ms, state.last_now_ms)
  state.last_now_ms = now_ms
  local phase, digit
  if state.cancelled then phase = "cancelled"
  elseif now_ms < state.countdown_at_ms then phase = "waiting"
  elseif now_ms < state.start_at_ms then
    phase = "countdown"
    digit = 3 - math.floor((now_ms - state.countdown_at_ms) / 1000)
  elseif now_ms < state.end_at_ms then phase = "running"
  else phase = "finished" end
  local previous = state.last_phase
  state.last_phase = phase
  local view = { match_id = state.match_id, phase = phase, digit = digit,
    input_allowed = phase == "running", content_visible = phase == "running",
    timer_seconds = phase == "running" and math.ceil((state.end_at_ms - now_ms) / 1000) or (phase == "finished" and 0 or 20),
    elapsed_ms = math.max(0, math.min(20000, now_ms - state.start_at_ms)),
    entered_running = phase == "running" and previous ~= "running",
    entered_finished = phase == "finished" and previous ~= "finished",
    start_at_ms = state.start_at_ms, end_at_ms = state.end_at_ms }
  if digit then
    local f = FRAMES[digit]
    -- Return a copy so UI mutation cannot change fixed geometry.
    view.frame = { asset = f.asset, x = f.x, y = f.y, width = f.width, height = f.height,
      center_x = 597, center_y = 476, scale = 1, opacity = 1 }
  end
  return view
end
function M.map_rect(frame, viewport_w, viewport_h)
  assert(finite(viewport_w) and finite(viewport_h) and viewport_w > 0 and viewport_h > 0, "positive viewport required")
  local q = math.min(viewport_w / 1600, viewport_h / 900)
  local ox, oy = (viewport_w - 1600 * q) / 2, (viewport_h - 900 * q) / 2
  return { x = ox + frame.x*q, y = oy + frame.y*q, width = frame.width*q, height = frame.height*q }
end
return M
