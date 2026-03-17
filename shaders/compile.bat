@echo off
set SCRIPT_DIR=%~dp0

for %%f in ("%SCRIPT_DIR%*.slang") do (
echo Compiling %%~nxf

C:\VulkanSDK\1.4.328.0\Bin\slangc.exe "%%f" ^
    -target spirv ^
    -profile spirv_1_4 ^
    -emit-spirv-directly ^
    -fvk-use-entrypoint-name ^
    -entry vertMain ^
    -entry fragMain ^
    -o "%SCRIPT_DIR%\%%~nf.spv"

)
