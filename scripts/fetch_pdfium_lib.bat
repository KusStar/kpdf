@echo off
setlocal EnableExtensions EnableDelayedExpansion

set "REPO=bblanchon/pdfium-binaries"
set "TAG_ALIAS=chromium"
set "TAG_ALIAS_ENCODED=chromium"

for %%I in ("%~dp0..") do set "ROOT_DIR=%%~fI"
set "OUTPUT_DIR=%ROOT_DIR%\lib"

set "TMP_DIR="
set "ARCHIVE="
set "EXTRACT_DIR="
set "ARCH="
set "TAG_ENCODED="
set "TAG_DISPLAY="
set "SELECTED_ASSET="
set "ERRMSG="
set "SHOW_HELP="

call :parse_args %*
if errorlevel 1 goto fail
if defined SHOW_HELP exit /b 0

where powershell >nul 2>nul || (
  set "ERRMSG=Missing required command: powershell"
  goto fail
)
where tar >nul 2>nul || (
  set "ERRMSG=Missing required command: tar"
  goto fail
)

call :detect_arch
if errorlevel 1 goto fail

call :resolve_tag
if errorlevel 1 goto fail

set "TMP_DIR=%TEMP%\kpdf_pdfium_%RANDOM%%RANDOM%"
set "ARCHIVE=%TMP_DIR%\pdfium.tgz"
set "EXTRACT_DIR=%TMP_DIR%\extract"

mkdir "%TMP_DIR%" >nul 2>nul || (
  set "ERRMSG=Failed to create temp directory: %TMP_DIR%"
  goto fail
)
mkdir "%EXTRACT_DIR%" >nul 2>nul || (
  set "ERRMSG=Failed to create extract directory: %EXTRACT_DIR%"
  goto fail
)

call :download_for_arch
if errorlevel 1 (
  if /I not "!TAG_ENCODED!"=="%TAG_ALIAS_ENCODED%" (
    set "TAG_ENCODED=%TAG_ALIAS_ENCODED%"
    set "TAG_DISPLAY=%TAG_ALIAS%"
    call :download_for_arch
  )
)
if errorlevel 1 goto fail

tar -xzf "%ARCHIVE%" -C "%EXTRACT_DIR%" >nul
if errorlevel 1 (
  set "ERRMSG=Failed to extract archive: %ARCHIVE%"
  goto fail
)

set "FOUND_LIB="
for /f "delims=" %%F in ('dir /s /b "%EXTRACT_DIR%\pdfium.dll" 2^>nul') do (
  set "FOUND_LIB=%%F"
  goto found_lib
)

:found_lib
if not defined FOUND_LIB (
  set "ERRMSG=pdfium.dll not found in downloaded archive."
  goto fail
)

if not exist "%OUTPUT_DIR%" mkdir "%OUTPUT_DIR%" >nul 2>nul
copy /Y "%FOUND_LIB%" "%OUTPUT_DIR%\pdfium.dll" >nul
if errorlevel 1 (
  set "ERRMSG=Failed to copy pdfium.dll to %OUTPUT_DIR%."
  goto fail
)

echo Resolved tag: !TAG_DISPLAY!
echo Downloaded: !SELECTED_ASSET!
echo Copied: %OUTPUT_DIR%\pdfium.dll
call :cleanup
exit /b 0

:parse_args
if "%~1"=="" exit /b 0

if /I "%~1"=="-h" (
  call :usage
  set "SHOW_HELP=1"
  exit /b 0
)
if /I "%~1"=="--help" (
  call :usage
  set "SHOW_HELP=1"
  exit /b 0
)

if /I "%~1"=="-o" (
  if "%~2"=="" (
    set "ERRMSG=Missing value for -o"
    exit /b 1
  )
  set "OUTPUT_DIR=%~2"
  shift
  shift
  goto parse_args
)
if /I "%~1"=="--output-dir" (
  if "%~2"=="" (
    set "ERRMSG=Missing value for --output-dir"
    exit /b 1
  )
  set "OUTPUT_DIR=%~2"
  shift
  shift
  goto parse_args
)

set "ERRMSG=Unknown argument: %~1"
exit /b 1

:usage
echo Usage: fetch_pdfium_lib.bat [options]
echo.
echo Download the latest PDFium binary for current Windows architecture from:
echo https://github.com/bblanchon/pdfium-binaries/releases/tag/chromium
echo.
echo Options:
echo   -o, --output-dir ^<dir^>   Output directory for pdfium.dll ^(default: .\lib^)
echo   -h, --help                 Show this help message
exit /b 0

:detect_arch
set "RAW_ARCH=%PROCESSOR_ARCHITECTURE%"
if defined PROCESSOR_ARCHITEW6432 set "RAW_ARCH=%PROCESSOR_ARCHITEW6432%"

if /I "%RAW_ARCH%"=="AMD64" (
  set "ARCH=x64"
  exit /b 0
)
if /I "%RAW_ARCH%"=="ARM64" (
  set "ARCH=arm64"
  exit /b 0
)
if /I "%RAW_ARCH%"=="X86" (
  set "ARCH=x86"
  exit /b 0
)

set "ERRMSG=Unsupported architecture: %RAW_ARCH%"
exit /b 1

:resolve_tag
set "TAG_RAW="
for /f "usebackq delims=" %%T in (`powershell -NoProfile -ExecutionPolicy Bypass -Command "$ErrorActionPreference='Stop'; try { $headers = @{ 'User-Agent' = 'kpdf-pdfium-fetch-bat/1.0' }; $releases = Invoke-RestMethod -Uri 'https://api.github.com/repos/%REPO%/releases?per_page=100' -Headers $headers; $tag = ($releases | ForEach-Object { $_.tag_name } | Where-Object { $_ -like 'chromium/*' } | Select-Object -First 1); if (-not $tag) { $tag = '%TAG_ALIAS%' }; $tag } catch { '%TAG_ALIAS%' }"`) do (
  set "TAG_RAW=%%T"
)
if not defined TAG_RAW set "TAG_RAW=%TAG_ALIAS%"
set "TAG_DISPLAY=%TAG_RAW%"

for /f "usebackq delims=" %%T in (`powershell -NoProfile -ExecutionPolicy Bypass -Command "[System.Uri]::EscapeDataString('%TAG_RAW%')"`) do (
  set "TAG_ENCODED=%%T"
)

if not defined TAG_ENCODED set "TAG_ENCODED=%TAG_ALIAS_ENCODED%"
exit /b 0

:download_for_arch
set "ASSET1="
set "ASSET2="

if /I "!ARCH!"=="x64" (
  set "ASSET1=pdfium-win-x64.tgz"
  set "ASSET2=pdfium-win-x86.tgz"
) else if /I "!ARCH!"=="arm64" (
  set "ASSET1=pdfium-win-arm64.tgz"
) else if /I "!ARCH!"=="x86" (
  set "ASSET1=pdfium-win-x86.tgz"
  set "ASSET2=pdfium-win-x64.tgz"
) else (
  set "ERRMSG=Unsupported architecture: !ARCH!"
  exit /b 1
)

call :try_download "!ASSET1!"
if not errorlevel 1 exit /b 0

if defined ASSET2 (
  call :try_download "!ASSET2!"
  if not errorlevel 1 exit /b 0
)

set "ERRMSG=Could not download matching asset for architecture !ARCH! under tag !TAG_ENCODED!."
exit /b 1

:try_download
set "ASSET=%~1"
if not defined ASSET exit /b 1

set "URL=https://github.com/%REPO%/releases/download/!TAG_ENCODED!/!ASSET!"
echo Trying tag/asset: !TAG_DISPLAY! / !ASSET!

powershell -NoProfile -ExecutionPolicy Bypass -Command "$ProgressPreference='SilentlyContinue'; try { Invoke-WebRequest -Uri '!URL!' -OutFile '!ARCHIVE!' -Headers @{ 'User-Agent' = 'kpdf-pdfium-fetch-bat/1.0' } -ErrorAction Stop; exit 0 } catch { exit 1 }" >nul
if errorlevel 1 exit /b 1

set "SELECTED_ASSET=!ASSET!"
exit /b 0

:cleanup
if defined TMP_DIR (
  if exist "%TMP_DIR%" rd /s /q "%TMP_DIR%" >nul 2>nul
)
exit /b 0

:fail
if not defined ERRMSG set "ERRMSG=Unknown error."
echo Error: %ERRMSG% 1>&2
call :cleanup
exit /b 1
