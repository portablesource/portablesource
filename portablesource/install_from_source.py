import os
import subprocess
import re
import locale
import winreg
from .downloader import get_install_path, download_for_main
import requests
from .get_gpu import get_gpu

install_path = get_install_path()
git_exe = os.path.join(install_path, 'system', 'git', 'cmd', 'git.exe')
python = os.path.join(install_path, 'system', 'python', 'python.exe')
ffmpeg = os.path.join(install_path, 'system', 'ffmpeg')
git_cmd = os.path.join(install_path, 'system', 'git', 'cmd')

repos = [
    "https://github.com/facefusion/facefusion",
    "https://github.com/KwaiVGI/LivePortrait",
    "https://github.com/lllyasviel/stable-diffusion-webui-forge",
    "https://github.com/comfyanonymous/ComfyUI",
    "https://github.com/hacksider/Deep-Live-Cam",
    "https://github.com/argenspin/Rope-Live",
]

for i, repo in enumerate(repos, 1):
    print(f"{i}. {repo}")

def create_venv(repo_path, python):
    venv_path = os.path.join(repo_path, "venv")
    if not os.path.exists(venv_path):
        subprocess.run([python, "-m", "venv", venv_path], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    python_venv = os.path.join(venv_path, "Scripts", "python.exe")
    python_venv_scripts = os.path.join(venv_path, "Scripts")
    return python_venv, python_venv_scripts, venv_path

def get_uv_path():
    scripts_dir = os.path.join(os.path.dirname(python), 'Scripts')
    uv_executable = os.path.join(scripts_dir, "uv.exe")
    return uv_executable

def install_uv():
    try:
        subprocess.run([python, "-m", "pip", "install", "uv"], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    except subprocess.CalledProcessError:
        return None
    uv_executable = get_uv_path()
    return uv_executable if os.path.exists(uv_executable) else None

def get_localized_text(language, key):
    texts = {
        "en": {
            "installed": "Installed! Try to use.",
            "select_repo": "Select a repository number or enter your reference:",
            "enter_requirements_filename": "Enter the name of the requirements file (press Enter for 'requirements.txt'): ",
        },
        "ru": {
            "installed": "Установлено! Надеюсь, вам понравится.",
            "select_repo": "Выберите номер репозитория или введите свою ссылку: ",
            "enter_requirements_filename": "Введите имя файла с библиотеками (нажмите Enter для 'requirements.txt'): ",
        }
    }
    return texts[language].get(key, "")

def get_system_language():
    try:
        key = winreg.OpenKey(winreg.HKEY_CURRENT_USER, r"Control Panel\International")
        language = winreg.QueryValueEx(key, "LocaleName")[0]
        winreg.CloseKey(key)
        lang_code = language.split('-')[0].lower()
        return "ru" if lang_code == "ru" else "en"
    except WindowsError:
        lang_code = locale.getlocale()[0].split('_')[0].lower()
        return "ru" if lang_code == "ru" else "en"

def install_requirements(venv_path, python_venv, uv_executable, requirements_file):
    if not os.path.exists(requirements_file):
        print(f"Requirements file not found: {requirements_file}")
        return

    installed_flag = os.path.join(venv_path, ".libraries_installed")
    
    if not os.path.exists(installed_flag):
        with open(requirements_file, 'r') as f:
            requirements = f.read()

        requirements = re.sub(r'(insightface).*\n', '', requirements)
        torch_packages = re.findall(r'(torch)', requirements)
        cuda_version_match = re.search(r'\+cu(\d+)', requirements)
        cuda_version = cuda_version_match.group(1) if cuda_version_match else None
        onnx_gpu = re.search(r'onnxruntime-gpu', requirements)
        
        with open(requirements_file, 'w') as f:
            f.write(requirements)
        
        install_cmd = f'"{python_venv}" && "{uv_executable}" pip install -r "{requirements_file}"'
        subprocess.run(install_cmd, shell=True, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        
        insightface_cmd = f'"{python_venv}" && "{uv_executable}" pip install https://huggingface.co/hanamizuki-ai/insightface-releases/resolve/main/insightface-0.7.3-cp310-cp310-win_amd64.whl'
        subprocess.run(insightface_cmd, shell=True, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        if onnx_gpu is not None:
            install_onnx_runtime(python_venv, uv_executable)
        if torch_packages:
            torch_cmd = f'"{python_venv}" && "{uv_executable}" pip install torch==2.4.0 torchvision==0.19.0 torchaudio==2.4.0 --index-url https://download.pytorch.org/whl/cu{cuda_version}"'
            subprocess.run(torch_cmd, shell=True, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

def install_onnx_runtime(python_venv, uv_executable):
    gpu = get_gpu()
    if gpu == "NVIDIA":
        ort_version, ort_lib_version = "onnxruntime-gpu", "1.18.0"
    elif gpu == "DIRECTML":
        ort_version, ort_lib_version = "onnxruntime-directml", "1.17.3"
    else:
        ort_version, ort_lib_version = "onnxruntime", "1.19.2"

    install_cmd = f'"{python_venv}" && "{uv_executable}" pip install {ort_version}=={ort_lib_version}"'
    subprocess.run(install_cmd, shell=True, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

def download_models(repo_name):
    if repo_name == "Deep-Live-Cam":
        models_dir = os.path.join(repo_name, "models")
        os.makedirs(models_dir, exist_ok=True)
        
        model_urls = [
            "https://huggingface.co/hacksider/deep-live-cam/resolve/main/GFPGANv1.4.pth",
            "https://github.com/facefusion/facefusion-assets/releases/download/models/inswapper_128_fp16.onnx"
        ]
        
        for url in model_urls:
            download_file(url, models_dir)

def download_file(url, destination_dir):
    filename = os.path.basename(url)
    local_path = os.path.join(destination_dir, filename)
    
    if not os.path.exists(local_path):
        response = requests.get(url, stream=True)
        response.raise_for_status()
        
        with open(local_path, 'wb') as file:
            for chunk in response.iter_content(chunk_size=16384):
                if chunk:
                    file.write(chunk)

def write_bat_file(repo_home, app_name, python_venv, python_venv_scripts):
    gpu = get_gpu()
    if gpu=="NVIDIA":
        system = os.path.join(install_path, "system")
        cuda_abs_path = os.path.join(system, "CUDA")
        cuda_bin = os.path.join(cuda_abs_path, "bin")
        cuda_lib = os.path.join(cuda_abs_path, "lib")
        cuda_include = os.path.join(cuda_abs_path, "include")
        cuda_libnvvp = os.path.join(cuda_abs_path, "libnvvp")
        cuda_for_add = f"{cuda_bin};{cuda_lib};{cuda_include};{cuda_libnvvp}"
        cuda_path = f'set "CUDA_PATH"={cuda_bin}'
    else:
        cuda_path = ""
        cuda_for_add = ""
    os.chdir(repo_home)
    tmp = os.path.join(repo_home, "tmp")
    os.makedirs(tmp, exist_ok=True)
    
    bat_content = f'''@echo off
setlocal enabledelayedexpansion
for /d %%i in (tmp\\tmp*,tmp\\pip*) do rd /s /q "%%i" 2>nul || ("%%i" && exit /b 1) & del /q tmp\\tmp* > nul 2>&1 & rd /s /q pip\\cache 2>nul

set "appdata={tmp}"
set "userprofile={tmp}"
set "temp={tmp}"
set "PATH=%PATH%;{cuda_for_add};{git_cmd};{python_venv};{python_venv_scripts};{ffmpeg};%PATH%"

set "CUDA_MODULE_LOADING=LAZY"
{cuda_path}

"{python_venv}" {app_name}
pause
endlocal
REM by dony
'''
    with open('start_nvidia.bat', 'w') as bat_file:
        bat_file.write(bat_content)

def facefusion_repo_setup(repo_url, abs_path):
    ff_path = os.path.join(abs_path, "sources", "facefusion")
    ff_master = os.path.join(ff_path, "master")
    os.makedirs(ff_path, exist_ok=True)
    
    if not os.path.exists(ff_master):
        os.chdir(ff_path)
        subprocess.run([git_exe, "clone", repo_url, "-b", "master", "master"], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    else:
        os.chdir(ff_master)
        subprocess.run([git_exe, "pull", "origin", "master"], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

def determine_app_name(repo_name):
    repo_apps = {
        "facefusion": "updater_facefusion.py",
        "Rope-Live": "Rope.py",
        "LivePortrait": "app.py",
        "ComfyUI": "main.py",
        "Deep-Live-Cam": "run.py",
        "stable-diffusion-webui-forge": "webui.py"
    }
    return repo_apps.get(repo_name, "app.py")

def download_updater_facefusion():
    updater_facefusion_url = "https://raw.githubusercontent.com/portablesource/portablesource/refs/heads/main/portablesource/updater_facefusion.py"
    updater_facefusion_name = "updater_facefusion.py"
    response = requests.get(updater_facefusion_url, stream=True)
    
    with open(updater_facefusion_name, 'wb') as out_file:
        for chunk in response.iter_content(chunk_size=1024):
            if chunk:
                out_file.write(chunk)

def install_from_source(language):
    choice = input(get_localized_text(language, "select_repo")).strip()

    if choice.isdigit() and 1 <= int(choice) <= len(repos):
        repo_url = repos[int(choice) - 1]
    else:
        repo_url = choice

    download_for_main()

    repo_name = repo_url.split('/')[-1].replace('.git', '')
    abs_path = get_install_path()
    sources_path = os.path.join(abs_path, "sources")
    repo_home = os.path.join(sources_path, repo_name)
    os.makedirs(repo_home, exist_ok=True)

    if repo_name != "facefusion":
        os.chdir(sources_path)
        repo_abs = os.path.join(sources_path, repo_name)
        if not os.path.exists(repo_abs):
            subprocess.run([git_exe, "clone", repo_url, repo_name], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        else:
            os.chdir(repo_abs)
            subprocess.run([git_exe, "pull", "origin"], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    else:
        facefusion_repo_setup(repo_url, abs_path)

    repo_path = os.path.join(abs_path, "sources", repo_name)
    python_venv, python_venv_scripts, venv_path = create_venv(repo_path, python)
    uv_executable = get_uv_path()

    requirements_file = os.path.join(repo_path, "master" if repo_name == "facefusion" else "", "requirements.txt")

    install_requirements(venv_path, python_venv, uv_executable, requirements_file)
    app_name = determine_app_name(repo_name)

    write_bat_file(repo_home, app_name, python_venv, python_venv_scripts)

    if repo_name == "facefusion":
        download_updater_facefusion()

def installed():
    language = get_system_language()
    install_from_source(language)