@ECHO OFF
SETLOCAL
SET PROJECT_DIR=%~dp0
IF EXIST "%~dp0gradlew.bat" (
  REM placeholder to keep symmetry with Unix helper
)
where gradle >NUL 2>&1
IF %ERRORLEVEL% EQU 0 (
  gradle --project-dir "%PROJECT_DIR%" %*
  GOTO END
)
ECHO Gradle executable not found. Install Gradle 8.2+ or run from Android Studio.
EXIT /B 1
:END
ENDLOCAL
