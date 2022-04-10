import shutil, os

paths = [
    "./css",
    "./img",
    "./js",
    "./templates",
]

for path in paths:
    for x in os.listdir(path):
        full_path = "%s/%s" % (path, x)
        shutil.copy(full_path, "./testing/")