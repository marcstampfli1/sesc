@echo off
"%~dp0target\release\peer-simulator.exe" --bind 0.0.0.0:28001 --peer 127.0.0.1:28000 --as client %*
