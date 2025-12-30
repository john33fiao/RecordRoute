' RecordRoute 실행 스크립트 (Windows - 콘솔 창 없이 실행)
' 이 스크립트는 콘솔 창을 표시하지 않고 RecordRoute를 시작합니다

Set WshShell = CreateObject("WScript.Shell")
Set fso = CreateObject("Scripting.FileSystemObject")

' 프로젝트 루트 디렉토리 (scripts 폴더의 부모 디렉토리)
scriptDir = fso.GetParentFolderName(WScript.ScriptFullName)
projectRoot = fso.GetParentFolderName(scriptDir)

' start.bat를 숨김 모드로 실행 (0 = 창 숨김, True = 완료 대기)
' Note: start.bat 파일이 존재하지 않으면 작동하지 않습니다
WshShell.Run """" & projectRoot & "\start.bat""", 0, False

Set WshShell = Nothing
Set fso = Nothing
