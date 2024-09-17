import os
import subprocess
import re
import urllib3
import zipfile
import sys
import shutil

git_exe = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'system', 'git', 'cmd', 'git.exe')
python = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'system', 'python', 'python.exe')
ffmpeg = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'system', 'ffmpeg')

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

def get_uv_path():
    if sys.platform.startswith('win'):
        scripts_dir = os.path.join(os.path.dirname(python), 'Scripts')
        uv_executable = os.path.join(scripts_dir, "uv.exe")
    else:
        scripts_dir = os.path.join(os.path.dirname(os.path.dirname(python)), 'bin')
        uv_executable = os.path.join(scripts_dir, "uv")
    return uv_executable

uv_executable = get_uv_path()

def install_uv():
    if shutil.which("uv") or os.path.exists(uv_executable):
        return uv_executable
    else:
        subprocess.run([python, "-m", "pip", "install", "uv"], check=True)
    

def get_localized_text(language, key):
    texts = {
        "en": {
              "select_repo": "Select a repository number or enter your reference:",
              "enter_requirements_filename": "Enter the name of the requirements file (press Enter for 'requirements.txt'): ",
        },
        "ru": {
             "select_repo": "Выберите номер репозитория или введите свою ссылку: ",
             "enter_requirements_filename": "Введите имя файла с библиотеками (нажмите Enter для 'requirements.txt'): ",
        }
    }
    return texts[language].get(key, "")

def read_language_from_file(file_path):
    if os.path.exists(file_path):
        with open(file_path, 'r', encoding='utf-8') as file:
            language = file.read().strip().lower()
            if language in ["en", "ru"]:
                return language
    return None

def write_language_to_file(file_path, language):
    with open(file_path, 'w', encoding='utf-8') as file:
        file.write(language)

def install_from_source(language):
    choice = input(get_localized_text(language, "select_repo")).strip()

    if choice.isdigit() and 1 <= int(choice) <= len(repos):
        repo_url = repos[int(choice) - 1]
    else:
        repo_url = choice

    repo_name = repo_url.split('/')[-1].replace('.git', '')
    os.makedirs(repo_name, exist_ok=True)

    if not os.path.exists(os.path.join(repo_name, '.git')):
        subprocess.run([git_exe, "clone", repo_url, repo_name], check=True)

    venv_path = os.path.join(repo_name, "venv")
    if not os.path.exists(venv_path):
        subprocess.run([python, "-m", "venv", venv_path], check=True)

    activate_script = os.path.join(venv_path, "Scripts", "activate.bat")

    requirements_filename = input(get_localized_text(language, "enter_requirements_filename")).strip()
    if not requirements_filename:
        requirements_filename = "requirements.txt"

    requirements_file = os.path.join(repo_name, requirements_filename)

    if os.path.exists(requirements_file):
        installed_flag = os.path.join(venv_path, ".libraries_installed")
        if not os.path.exists(installed_flag):
            with open(requirements_file, 'r') as f:
                requirements = f.read()
        
        torch_packages = re.findall(r'(torch|torchvision|torchaudio)', requirements)
        onnx_gpu = re.search(r'onnxruntime-gpu', requirements)
        
        if torch_packages:
            torch_cmd = f'"{activate_script}" && "{python}" -m {uv_executable} pip install {" ".join(torch_packages)}'
            subprocess.run(torch_cmd, shell=True, check=True)

        if onnx_gpu:
            onnx_url = "https://huggingface.co/datasets/NeuroDonu/PortableSource/resolve/main/onnxruntime-gpu.zip"
            onnx_zip = os.path.join(repo_name, "onnxruntime-gpu.zip")
            venv_lib_path = os.path.join(repo_name, "venv", "Lib", "site-packages")
            http = urllib3.PoolManager()
            
            with http.request('GET', onnx_url, preload_content=False) as resp, open(onnx_zip, 'wb') as out_file:
                while True:
                    data = resp.read(1024)
                    if not data:
                        break
                    out_file.write(data)

                    with zipfile.ZipFile(onnx_zip, 'r') as zip_ref:
                        zip_ref.extractall(venv_lib_path)
                    os.remove(onnx_zip)
        
        install_cmd = f'"{activate_script}" && "{python}" -m {uv_executable} pip install -r "{requirements_file}"'
        subprocess.run(install_cmd, shell=True, check=True)
        
        open(installed_flag, 'w').close()

def installed():
    file_path = 'lang.txt'
    language = read_language_from_file(file_path)
    if not language:
        language = input(get_localized_text("en", "choose_language")).strip().lower()
        if language not in ["en", "ru"]:
            language = "en"
        write_language_to_file(file_path, language)
    install_from_source()

