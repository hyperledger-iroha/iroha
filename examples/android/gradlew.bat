@ECHO OFF
SETLOCAL
CALL "%~dp0..\..\kotlin\gradlew.bat" --project-dir "%~dp0" %*
EXIT /B %ERRORLEVEL%
