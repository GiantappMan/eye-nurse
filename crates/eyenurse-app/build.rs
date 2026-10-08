//! Windows 资源构建脚本（实现者 D 所有，架构文档 §1）。
//!
//! 1) 资源嵌入（§7.5）：生成临时 .rc（RT_MANIFEST = 类型 24 / id 1 + 图标
//!    IDI_ICON1），经 embed-resource 调 rc.exe（MSVC 环境自带）链入 exe——
//!    图标 = 仓库根 `assets/logo.ico`，manifest = 包根 `app.manifest`（PerMonitorV2）；
//! 2) 一致性差异 #8（清单 H1）：cargo build 时把单一真源
//!    `crates/eyenurse-core/assets/languages/{zh,en}.json` 拷到
//!    `target/{profile}/Assets/Languages/`——dev 运行即满足 v2 的“exe 旁
//!    Assets/Languages、替换 json 即换文案”布局；安装包同源拷贝（setup/compile.ps1）。
//!    拷贝为 best-effort：失败不阻塞构建（运行时 load_i18n 回退内嵌副本）。

use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=app.manifest");
    println!("cargo:rerun-if-changed=../../assets/logo.ico");
    println!("cargo:rerun-if-changed=../eyenurse-core/assets/languages/zh.json");
    println!("cargo:rerun-if-changed=../eyenurse-core/assets/languages/en.json");

    copy_languages_next_to_exe();
    embed_windows_resources();
}

/// OUT_DIR = target/{profile}/build/eyenurse-app-<hash>/out，三级祖先即 {profile} 目录
fn target_profile_dir() -> Option<PathBuf> {
    let out = PathBuf::from(std::env::var_os("OUT_DIR")?);
    out.ancestors().nth(3).map(Path::to_path_buf)
}

fn copy_languages_next_to_exe() {
    let Some(dest_root) = target_profile_dir() else {
        return;
    };
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default());
    let src_dir = manifest_dir.join("../eyenurse-core/assets/languages");
    let dest_dir = dest_root.join("Assets").join("Languages");
    if std::fs::create_dir_all(&dest_dir).is_ok() {
        for name in ["zh.json", "en.json"] {
            let _ = std::fs::copy(src_dir.join(name), dest_dir.join(name));
        }
    }
}

fn embed_windows_resources() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default());
    let Some(out_dir) = std::env::var_os("OUT_DIR").map(PathBuf::from) else {
        return;
    };
    let icon = manifest_dir.join("../../assets/logo.ico");
    let manifest = manifest_dir.join("app.manifest");
    if !icon.is_file() || !manifest.is_file() {
        // 缺资源不阻塞构建（与语言拷贝同策略；正仓库两文件恒在）
        return;
    }
    // .rc 语法：`<id> <RT_MANIFEST=24> "<path>"` + `<名称> ICON "<path>"`。
    // 路径一律用正斜杠：rc.exe 会把 `\a` 等反斜杠序列当转义吃掉
    // （实测 `...\eyenurse-app\app.manifest` 被读成 `...eyenurse-appp.manifest`）
    let rc = out_dir.join("eyenurse.rc");
    let manifest_s = manifest.to_string_lossy().replace('\\', "/");
    let icon_s = icon.to_string_lossy().replace('\\', "/");
    std::fs::write(
        &rc,
        format!("1 24 \"{manifest_s}\"\r\nIDI_ICON1 ICON \"{icon_s}\"\r\n"),
    )
    .unwrap();
    // DPI 声明功能上必需 → manifest_required：缺失时链接期报错而非静默
    embed_resource::compile(&rc, embed_resource::NONE)
        .manifest_required()
        .unwrap();
}
