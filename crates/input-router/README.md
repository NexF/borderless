# borderless-input-router

虚拟屏幕拓扑、越界检测、修饰键状态机。**纯逻辑、零 IO**。

v0.2 起仅 Hub 端使用本 crate：Hub PAL 上来的原始 `InputEvent` 配合用户
配置的 `Layout`，由本 crate 决定每条事件应该"留在 Hub 本地"还是"转发
给某个 Spoke"。Spoke 不跑 capture，因此不会调本 crate。

## 模块

| 模块 | 作用 |
|---|---|
| `layout` | 屏幕矩形 + 边邻接表（左右上下连到谁）|
| `modifier` | 修饰键状态机（`Shift`/`Ctrl`/`Alt`/`Cmd` 的实时按压位掩码）|
| `router` | 真正干活：根据当前 active 节点 + 鼠标/按键事件，产出 `Routed::{Local, Remote}` 决策 |

## 关键不变量

- 每次鼠标 delta 应用后判断是否越界；越过有邻居的边 → 发出
  `(Leave, Enter)` 一对，`Enter.modifiers` 携带"当前所有按住的修饰键"，
  避免 `Shift` 等修饰键在跨屏时撕裂
- 越过没有邻居的边 → 夹紧（clamp）到屏幕边沿，事件本地分发
- `active` 节点 ≠ self 时，所有键鼠事件直接转发给 active

## 测试

12 个单元测试覆盖各种边界：

- 边对称（A 的右 = B 的左）
- 修饰键状态机（press/release、组合键）
- 鼠标越界生成 `(Leave, Enter)`、修饰键随 `Enter` 携带
- active=remote 时按键被转发，active=self 时本地处理
- 越过未连接的边时正确夹紧、不 panic、不误转发
