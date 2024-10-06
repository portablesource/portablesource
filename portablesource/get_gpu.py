import subprocess
import platform

def get_gpu():
    system = platform.system()
    gpus = []

    if system == "Windows":
        output = subprocess.check_output(["wmic", "path", "win32_VideoController", "get", "name"]).decode("utf-8")
        gpus = [line.strip() for line in output.splitlines() if line.strip() and line.strip() != "Name"]
    elif system == "Linux":
        output = subprocess.check_output(["lspci"]).decode("utf-8")
        gpus = [line for line in output.splitlines() if "VGA" in line or "3D" in line]
    
    for gpu in gpus:
        if "NVIDIA" in gpu.upper():
            return "NVIDIA"
        elif "INTEL" in gpu.upper():
            return "DIRECTML"
        elif "AMD" in gpu.upper():
            return "DIRECTML"

    return "CPU"