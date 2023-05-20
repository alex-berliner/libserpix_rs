@echo off

set "url=https://github.com/alex-berliner/LibSerpix/archive/refs/heads/main.zip"
set "outputFile=LibSerpix.zip"
set "extractDir=LibSerpix"

@REM REM Download the file
@REM curl -o "%outputFile%" -L "%url%"

REM Extract the file
powershell -Command "Expand-Archive -Path '%outputFile%' -DestinationPath '%extractDir%' -Force"

