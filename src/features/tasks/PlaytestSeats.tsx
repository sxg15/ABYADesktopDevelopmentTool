import "./playtestSeats.css";

export function PlaytestSeats({ value, onChange, disabled, en = false }: {
  value: number;
  onChange: (value: number) => void;
  disabled: boolean;
  en?: boolean;
}) {
  return <div className="playtest-seats">
    <label>{en ? "Players (windows)" : "试玩人数（窗口数）"}
      <select value={value} disabled={disabled} onChange={event => onChange(Number(event.target.value))}>
        {Array.from({ length: 8 }, (_, index) => index + 1).map(count =>
          <option key={count} value={count}>{en ? `${count} player${count > 1 ? "s" : ""} · ${count} window${count > 1 ? "s" : ""}` : `${count} 人 · ${count} 个窗口`}</option>)}
      </select>
    </label>
    <small>{en
      ? "Each window is one human player; the first hosts the room. Choose within the game's capacity. Add bots in-game."
      : "每个窗口代表一名真人，首个窗口为房主。请按游戏人数上限选择，Bot 在游戏内添加。"}</small>
  </div>;
}
