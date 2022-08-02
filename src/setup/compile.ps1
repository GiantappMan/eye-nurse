# build app-installer with one-click

function DeletePath([String]$path) {
    if (Test-Path $path) {
        Remove-Item -path $path -Recurse
        Write-Host ("delete path {0}..." -f $path)
    }
}

# build frontend
Set-Location ../eyenurse-client-ui
yarn
yarn build

# map frontend dist to client ui folder
Set-Location ../tools
./LinkWebFolder.bat

Set-Location ../setup

Import-Module -Name "$PSScriptRoot\Invoke-MsBuild\Invoke-MsBuild.psm1" -Force

$sln = "..\EyeNurse\EyeNurse.csproj"
$buildDist = "$PSScriptRoot\publish"
# clean dist
DeletePath -path $buildDist
# build sln
# Invoke-MsBuild -Path $sln -MsBuildParameters "-t:restore /target:Clean;Build /property:Configuration=Release;OutputPath=$buildDist" -ShowBuildOutputInNewWindow -PromptForInputBeforeClosing -AutoLaunchBuildLogOnFailure
Invoke-MsBuild -Path $sln -MsBuildParameters "-t:restore /target:Clean;Publish /p:PublishProfile=./FolderProfile.pubxml" -ShowBuildOutputInNewWindow -PromptForInputBeforeClosing -AutoLaunchBuildLogOnFailure
