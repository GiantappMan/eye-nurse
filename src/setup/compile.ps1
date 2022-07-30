# build app-installer with one-click

function DeletePath([String]$path) {
    if (Test-Path $path) {
        Remove-Item -path $path -Recurse
        Write-Host ("delete path {0}..." -f $path)
    }
}

Import-Module -Name "$PSScriptRoot/Invoke-MsBuild/Invoke-MsBuild.psm1" -Force

$sln = "../EyeNurse2.sln"
$buildDist = "$PSScriptRoot/publish"
# clean dist
DeletePath -path $buildDist
# build sln
Invoke-MsBuild -Path $sln -Params "/property:Configuration=Release;OutputPath=$buildDist" -ShowBuildOutputInNewWindow -PromptForInputBeforeClosing -AutoLaunchBuildLogOnFailure
