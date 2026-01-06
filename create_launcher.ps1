# Download Bat to Exe Converter
$url = "https://github.com/islamadel/bat2exe/releases/download/v1.7/Bat_To_Exe_Converter.exe"
$output = "Bat_To_Exe_Converter.exe"

Write-Host "Downloading Bat to Exe Converter..."
Invoke-WebRequest -Uri $url -OutFile $output

Write-Host "Converting launcher.bat to InterviewHelper.exe..."
.\Bat_To_Exe_Converter.exe /bat launcher.bat /exe InterviewHelper.exe /icon icon.ico /invisible

Write-Host "Done! InterviewHelper.exe created."
