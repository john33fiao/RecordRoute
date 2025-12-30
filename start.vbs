' RecordRoute 실행 스크립트 (Windows - 콘솔 창 없이 실행)
' 이 스크립트는 콘솔 창을 표시하지 않고 RecordRoute를 시작합니다

Set WshShell = CreateObject("WScript.Shell")
Set fso = CreateObject("Scripting.FileSystemObject")

' 스크립트가 있는 디렉토리로 이동
scriptDir = fso.GetParentFolderName(WScript.ScriptFullName)

' start.bat를 숨김 모드로 실행 (0 = 창 숨김, True = 완료 대기)
WshShell.Run """" & scriptDir & "\start.bat""", 0, False

Set WshShell = Nothing
Set fso = Nothing
