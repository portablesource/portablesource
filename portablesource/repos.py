repositories = {
    "https://github.com/facefusion/facefusion": {
        "main_branch": "master",
        "executable": "updater_facefusion.py",
        "nvidia_libraries": [
            "filetype==1.2.0", 
            "gradio==4.44.0", 
            "gradio-rangeslider==0.0.6", 
            "numpy==1.26.4", 
            "onnx==1.16.1", 
            "onnxruntime-gpu==1.18.0", 
            "opencv-python==4.10.0.84", 
            "psutil==6.0.0", 
            "tqdm==4.66.5", 
            "scipy==1.14.1"],
        "directml_libraries": [
            "filetype==1.2.0", 
            "gradio==4.44.0", 
            "gradio-rangeslider==0.0.6", 
            "numpy==1.26.4", 
            "onnx==1.16.1", 
            "onnxruntime-directml==1.17.3", 
            "opencv-python==4.10.0.84", 
            "psutil==6.0.0", 
            "tqdm==4.66.5", 
            "scipy==1.14.1"],
        "torch": None,
        "torchvision": None,
        "torchaudio": None,
        "torch_index": None,

    },
    "https://github.com/KwaiVGI/LivePortrait": {
        "main_branch": "main",
        "executable": "app.py",
        "nvidia_libraries": ["onnxruntime-gpu==1.18.0", "transformers==4.22.0"],
        "directml_libraries": ["onnxruntime-directml==1.17.3", "transformers==4.22.0"]
    },
    "https://github.com/lllyasviel/stable-diffusion-webui-forge": {
        "main_branch": "main",
        "executable": "webui.py",
        "nvidia_libraries": ["torch==2.4.0", "torchvision==0.19.0"],
        "directml_libraries": ["onnxruntime-directml==1.17.3"]
    },
}