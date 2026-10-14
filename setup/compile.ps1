# v3 一键打包（一致性差异 #8 / 清单 J4；对齐 v2 src/setup/compile.ps1 的职责）：
# cargo release 构建 → 暂存 publish/（exe + Assets/Languages，与 build.rs 的
# cargo dev 拷贝同源：crates/eyenurse-core/assets/languages/）→
# 检测到 Inno Setup（ISCC.exe）时生成安装包，否则输出绿色包并提示。
$ErrorActionPreference = "Stop"

$root = Resolve-Path "$PSScriptRoot/.."
Set-Location $root

# 1) tauri build（先建 ui/dist 前端，再 cargo release 并推入 custom-protocol
#    资产嵌入；bundle.active=false 不出安装器——裸 `cargo build --release` 不带
#    custom-protocol 时不嵌页面，必须经 tauri CLI，tauri 架构文档 §1.3/§10）
bun run app:build

# 2) 暂存绿色包
$publish = "$PSScriptRoot/publish"
if (Test-Path $publish) { Remove-Item $publish -Recurse -Force }
New-Item -ItemType Directory -Path "$publish/Assets/Languages" -Force | Out-Null
Copy-Item "$root/target/release/eyenurse.exe" "$publish/"
Copy-Item "$root/crates/eyenurse-core/assets/languages/zh.json" "$publish/Assets/Languages/"
Copy-Item "$root/crates/eyenurse-core/assets/languages/en.json" "$publish/Assets/Languages/"

# 3) 安装包（可选：装有 Inno Setup 才执行）
$iscc = Get-Command ISCC.exe -ErrorAction SilentlyContinue
if ($iscc) {
    & $iscc.Source "$PSScriptRoot/installer.iss"
} else {
    Write-Host "未检测到 Inno Setup（ISCC.exe）：跳过安装包，绿色包在 $publish"
}
