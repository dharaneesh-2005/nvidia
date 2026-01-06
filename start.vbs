Set WshShell = CreateObject("WScript.Shell")
Set fso = CreateObject("Scripting.FileSystemObject")
scriptDir = fso.GetParentFolderName(WScript.ScriptFullName)
WshShell.CurrentDirectory = scriptDir
WshShell.Run Chr(34) & scriptDir & "\start_hidden.bat" & Chr(34), 0, False
Set WshShell = Nothing
Set fso = Nothing
