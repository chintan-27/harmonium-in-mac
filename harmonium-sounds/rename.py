import os
from pathlib import Path
sound_dir = Path("sounds")
new_sound_dir = Path("harmonium-sounds")

assert os.path.isdir(sound_dir), "Folder not found"

os.makedirs(new_sound_dir, exist_ok=True)

for old_file_name in os.listdir(sound_dir):
    new_file_name = old_file_name.split('-')[1]
    try:
        os.rename(sound_dir / old_file_name, new_sound_dir / new_file_name)
        print(f"File '{old_file_name}' renamed to '{new_file_name}' successfully.")
    except FileNotFoundError:
        print(f"Error: The file '{old_file_name}' was not found.")
    except FileExistsError:
        print(f"Error: A file named '{new_file_name}' already exists.")
    except Exception as e:
        print(f"An unexpected error occurred: {e}")