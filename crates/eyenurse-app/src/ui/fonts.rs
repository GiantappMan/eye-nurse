//! CJK 字体加载（实现者 C 所有，架构文档 §1 契约 / §7.5）。
//!
//! 签名冻结：`load_cjk_fonts(ctx: &egui::Context)`——**由 B 在 `AppModel::new` 里
//! 首帧前调用一次**；D 的设置窗依赖它但不得自行调用，避免重复插入字体。

/// egui 默认字体无汉字。启动时读 `C:\Windows\Fonts\msyh.ttc`（微软雅黑，失败依次尝试
/// `simsun.ttc`、`simhei.ttf`），插入 "cjk" 并追加到 Proportional/Monospace 家族尾部。
pub fn load_cjk_fonts(ctx: &egui::Context) {
    const CANDIDATES: [&str; 3] = [
        r"C:\Windows\Fonts\msyh.ttc",
        r"C:\Windows\Fonts\simsun.ttc",
        r"C:\Windows\Fonts\simhei.ttf",
    ];
    for path in CANDIDATES {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        let mut fonts = egui::FontDefinitions::default();
        // ttc 面号 0（字段已核实存在）；tweak 默认
        let data = egui::FontData {
            font: bytes.into(),
            index: 0,
            tweak: Default::default(),
        };
        fonts
            .font_data
            .insert("cjk".to_owned(), std::sync::Arc::new(data));
        // 追加到家族尾部：汉字命中 cjk，拉丁字符仍走默认字体
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .push("cjk".to_owned());
        fonts
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .push("cjk".to_owned());
        ctx.set_fonts(fonts);
        return;
    }
    // 三个候选都不存在：保持默认字体（罕见：精简版系统），中文显示为占位方块但不崩溃
}
