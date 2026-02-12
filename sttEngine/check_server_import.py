import sys
from pathlib import Path
import os
import inspect

# Add current dir to path
sys.path.append(os.getcwd())

try:
    from server import search_vectors
except ImportError as e:
    print(f"ImportError: {e}")
    sys.exit(1)

print(f"Imported search_vectors: {search_vectors}")
print(f"Type: {type(search_vectors)}")

try:
    sig = inspect.signature(search_vectors)
    print(f"Signature: {sig}")
except ValueError:
    print("Could not get signature")

print(f"Module: {search_vectors.__module__}")

# Try calling it
try:
    print("Calling with 1 arg...")
    search_vectors("test")
except Exception as e:
    print(f"Error 1 arg: {e}")

try:
    print("Calling with 3 args...")
    # Mock base_dir
    base_dir = Path("test")
    search_vectors("test", base_dir, 6)
    print("Success 3 args")
except Exception as e:
    print(f"Error 3 args: {e}")
