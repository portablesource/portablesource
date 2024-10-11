import os
import subprocess
import locale
import winreg
import argparse

def get_localized_text(language, key):
    texts = {
        "en": {
            "choose_action": "Choose an action:",
            "update_master": "1. Update to the master branch and start it",
            "update_next": "2. Update to the next branch and start it",
            "enter_choice": "Enter the number of the action: ",
            "invalid_choice": "Invalid choice, please try again.",
            "enable_webcam": "Enable webcam mode? (Y/N): ",
            "which_path": "Select an installation path or enter your reference, default C:\\ :",
        },
        "ru": {
            "choose_action": "Выберите действие:",
            "update_master": "1. Обновить до обычной ветки и запустить её (master)",
            "update_next": "2. Обновить до бета ветки и запустить её (next)",
            "enter_choice": "Введите номер действия: ",
            "invalid_choice": "Неверный выбор, попробуйте снова.",
            "enable_webcam": "Включить режим вебкамеры? (Y/N): ",
            "which_path": "Выберите путь установки, дефолтный C:\\ :",
        }
    }
    return texts[language].get(key, "")

def get_installed_path():
    try:
        key = winreg.OpenKey(winreg.HKEY_CURRENT_USER, r"Environment", 0, winreg.KEY_READ)
        path, _ = winreg.QueryValueEx(key, "Path")
        winreg.CloseKey(key)
        paths = path.split(';')
        for p in paths:
            if 'portablesource' in p and os.path.exists(os.path.join(p, 'installed.txt')):
                return p
        env_path = os.environ.get('PATH', '')
        env_paths = env_path.split(os.pathsep)
        for p in env_paths:
            if 'portablesource' in p and os.path.exists(os.path.join(p, 'installed.txt')):
                return p
        return None
    except Exception:
        return None

def get_system_language():
    try:
        key = winreg.OpenKey(winreg.HKEY_CURRENT_USER, r"Control Panel\International")
        language = winreg.QueryValueEx(key, "LocaleName")[0]
        winreg.CloseKey(key)
        lang_code = language.split('-')[0].lower()
        return "ru" if lang_code == "ru" else "en"
    except WindowsError:
        lang_code = locale.getdefaultlocale()[0].split('_')[0].lower()
        return "ru" if lang_code == "ru" else "en"

def get_path_for_install():
    language = get_system_language()
    if language not in ["en", "ru"]:
        language = "en"
    p = get_installed_path()
    if p is None:
        default_path = "C:\\"
        user_input = input(get_localized_text(language, "which_path")).strip()
        p = user_input if user_input else default_path
    return p

abs_path = get_path_for_install()
git = os.path.join(abs_path, "system", "git", "cmd", "git.exe")
ff_obs = os.path.join(abs_path, "sources", "facefusion")
python = os.path.join(abs_path, "sources", "facefusion", "venv", "Scripts", "python.exe")
parser = argparse.ArgumentParser

parser.add_argument('-wf', '--webcam-false', action='store_true', help="Выключить режим веб-камеры")
parser.add_argument('-wt', '--webcam-true', action='store_true', help="Включить режим веб-камеры")
parser.add_argument('-mb', '--master-branch', action='store_true', help="Лончить лишь мастер-ветку")
parser.add_argument('-nb', '--next-branch', action='store_true', help="Лончить лишь бета-ветку")

args = parser.parse_args()

def run_git_command(args):
    subprocess.run([git] + args, check=True)

def branch_path(branch):
    branch_path = os.path.join(ff_obs, branch)
    return branch_path

def uncensore(branch):
    file_path = os.path.join(ff_obs, branch, "facefusion", "content_analyser.py")
    
    with open(file_path, 'r', encoding='utf-8') as file:
        lines = file.readlines()

    modified_lines = []
    inside_function = False
    current_function = None
    functions_to_modify = {
        "def analyse_frame(vision_frame : VisionFrame) -> bool:": "def analyse_frame(vision_frame) -> bool:",
        "def forward(vision_frame : VisionFrame) -> float:": "def forward(vision_frame) -> float:",
        "def prepare_frame(vision_frame : VisionFrame) -> VisionFrame:": "def prepare_frame(vision_frame) -> VisionFrame:"
    }

    for line in lines:
        if any(func in line for func in functions_to_modify.keys()):
            inside_function = True
            current_function = next(func for func in functions_to_modify if func in line)
            modified_lines.append(line.replace(current_function, functions_to_modify[current_function]))
        elif inside_function:
            if "return" in line:
                modified_lines.append("    return False\n")
                inside_function = False
        else:
            modified_lines.append(line)

    with open(file_path, 'w', encoding='utf-8') as file:
        file.writelines(modified_lines)

def update_branch(branch):
    path = branch_path(branch)
    os.chdir(path)
    run_git_command(['reset', '--hard'])
    run_git_command(['checkout', branch])
    run_git_command(['pull', 'origin', branch, '--rebase'])

def start_ff(branch, webcam_mode=False):
    path = branch_path(branch)
    second_file = os.path.join(path, "facefusion.py")
    args = ["run", "--open-browser"]
    if webcam_mode:
        args.append("--ui-layouts")
        args.append("webcam")
    cmd = f'"{python}" "{second_file}" {" ".join(args)}'
    subprocess.run(cmd, shell=True, check=True)

def ask_webcam_mode(language):
    while True:
        webcam_choice = input(get_localized_text(language, "enable_webcam")).strip().lower()
        if webcam_choice in ["y", "yes", "да", "д"]:
            return True
        elif webcam_choice in ["n", "no", "нет", "н", ""]:
            return False
        else:
            return False

def facefusion():
    language = get_system_language()
    if args.master_branch:
        update_branch("master")
        uncensore("master")
        if args.webcam_true:
            webcam_mode = True
        elif args.webcam_false:
            webcam_mode = False
        else:
            webcam_mode = ask_webcam_mode(language)
        
        start_ff("master", webcam_mode)
        return 
    
    if args.next_branch:
        update_branch("next")
        uncensore("next")
        if args.webcam_true:
            webcam_mode = True
        elif args.webcam_false:
            webcam_mode = False
        else:
            webcam_mode = ask_webcam_mode(language)
        
        start_ff("next", webcam_mode)
        return 
    else:
        while True:
            print(get_localized_text(language, "choose_action"))
            print(get_localized_text(language, "update_master"))
            print(get_localized_text(language, "update_next"))
            choice = input(get_localized_text(language, "enter_choice")).strip()

            if choice == '1':
                update_branch("master")
                webcam_mode = ask_webcam_mode(language)
                uncensore("master")
                start_ff("master", webcam_mode)
            elif choice == '2':
                update_branch("next")
                uncensore("next")
                webcam_mode = ask_webcam_mode(language)
                start_ff("next", webcam_mode)
            else:
                print(get_localized_text(language, "invalid_choice"))

if __name__ == "__main__":
    facefusion()