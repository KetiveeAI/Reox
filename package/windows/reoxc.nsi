# NSIS Installer Script for REOX Compiler
# Build with: makensis reoxc.nsi

!include "MUI2.nsh"
!include "EnvVarUpdate.nsh"

# Installer info
Name "REOX Compiler"
OutFile "reoxc-setup.exe"
InstallDir "$PROGRAMFILES64\Reox"
InstallDirRegKey HKLM "Software\KetiveeAI\Reox" "InstallDir"
RequestExecutionLevel admin

# Version info
VIProductVersion "0.5.0.0"
VIAddVersionKey "ProductName" "REOX Compiler"
VIAddVersionKey "CompanyName" "KetiveeAI"
VIAddVersionKey "FileDescription" "REOX Language Compiler Installer"
VIAddVersionKey "FileVersion" "0.5.0-beta"
VIAddVersionKey "ProductVersion" "0.5.0-beta"
VIAddVersionKey "LegalCopyright" "Copyright (c) 2025 KetiveeAI"

# Modern UI settings
!define MUI_ICON "assets\reox.ico"
!define MUI_UNICON "assets\reox.ico"
!define MUI_HEADERIMAGE
!define MUI_WELCOMEFINISHPAGE_BITMAP "assets\welcome.bmp"

# Pages
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "..\..\LICENSE"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

# Installation
Section "REOX Compiler" SecMain
    SetOutPath "$INSTDIR\bin"
    File "..\..\dist\windows-x64\reoxc.exe"
    
    SetOutPath "$INSTDIR\runtime\include"
    File "..\..\reox-lang\runtime\*.h"
    
    # Registry
    WriteRegStr HKLM "Software\KetiveeAI\Reox" "InstallDir" "$INSTDIR"
    WriteRegStr HKLM "Software\KetiveeAI\Reox" "Version" "0.5.0-beta"
    
    # Add to PATH
    ${EnvVarUpdate} $0 "PATH" "A" "HKLM" "$INSTDIR\bin"
    
    # File associations
    WriteRegStr HKCR ".rx" "" "ReoxSourceFile"
    WriteRegStr HKCR ".reox" "" "ReoxSourceFile"
    WriteRegStr HKCR "ReoxSourceFile" "" "REOX Source File"
    WriteRegStr HKCR "ReoxSourceFile\DefaultIcon" "" "$INSTDIR\bin\reoxc.exe,0"
    
    # Uninstaller
    WriteUninstaller "$INSTDIR\Uninstall.exe"
    
    # Add/Remove Programs entry
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Reox" \
                     "DisplayName" "REOX Compiler"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Reox" \
                     "UninstallString" "$INSTDIR\Uninstall.exe"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Reox" \
                     "DisplayVersion" "0.5.0-beta"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Reox" \
                     "Publisher" "KetiveeAI"
SectionEnd

# Uninstallation
Section "Uninstall"
    # Remove files
    Delete "$INSTDIR\bin\reoxc.exe"
    RMDir /r "$INSTDIR\runtime"
    Delete "$INSTDIR\Uninstall.exe"
    RMDir "$INSTDIR\bin"
    RMDir "$INSTDIR"
    
    # Remove from PATH
    ${un.EnvVarUpdate} $0 "PATH" "R" "HKLM" "$INSTDIR\bin"
    
    # Remove registry
    DeleteRegKey HKLM "Software\KetiveeAI\Reox"
    DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Reox"
    DeleteRegKey HKCR ".rx"
    DeleteRegKey HKCR ".reox"
    DeleteRegKey HKCR "ReoxSourceFile"
SectionEnd
