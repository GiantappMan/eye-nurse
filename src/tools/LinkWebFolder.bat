Call :Link "..\EyeNurse\Assets\UI" "..\eyenurse-client-ui\dist"

Rem Link distfolder sourcefolder
:Link 
if [%1]==[] exit /b 0

rmdir /s /q %1
mklink /j %1 %2

pause
EXIT /B 0

