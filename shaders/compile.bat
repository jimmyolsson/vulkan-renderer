@echo off
set SCRIPT_DIR=%~dp0
C:\VulkanSDK\1.4.328.0\Bin\slangc.exe "%SCRIPT_DIR%\shader.slang" -target spirv -profile spirv_1_4 -emit-spirv-directly -fvk-use-entrypoint-name -entry vertMain -entry fragMain -o "%SCRIPT_DIR%\slang.spv"
