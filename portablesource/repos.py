repositories = {
    "https://github.com/facefusion/facefusion": {
        "main_branch": "master",
        "executable": "updater_facefusion.py",
        "main_lib" : [
            "filetype==1.2.0", 
            "gradio==4.44.0", 
            "gradio-rangeslider==0.0.6", 
            "numpy==1.26.4", 
            "onnx==1.16.1", 
            "onnxruntime-gpu==1.18.0", 
            "opencv-python==4.10.0.84", 
            "psutil==6.0.0", 
            "tqdm==4.66.5", 
            "scipy==1.14.1"
        ],
        "nvidia_libraries": [ 
            "onnxruntime-gpu==1.18.0"],
        "directml_libraries": [
            "onnxruntime-directml==1.17.3"],
        "torch": None,
        "torchvision": None,
        "torchaudio": None,
        "torch_index": None,
    },
}