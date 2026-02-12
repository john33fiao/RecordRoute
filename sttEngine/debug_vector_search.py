import sys
from pathlib import Path
import inspect
import os

# Add sttEngine to path
current_dir = Path(__file__).parent.resolve()
sys.path.append(str(current_dir))

try:
    from sttEngine.vector_search import search
except ImportError:
    try:
        from vector_search import search
    except ImportError:
        print("Could not import vector_search")
        sys.exit(1)

print(f"Type: {type(search)}")
try:
    print(f"Signature: {inspect.signature(search)}")
except ValueError:
    print("Signature: Could not get signature")

print(f"Doc: {search.__doc__}")

try:
    print("Attempting call with 1 arg...")
    search("test")
except Exception as e:
    print(f"Error calling with 1 arg: {e}")

try:
    print("Attempting call with 3 args...")
    # Mock embedding_pipeline attributes if needed, but we just want to check signature dispatch
    search("test", Path("."), 1)
    print("Success calling with 3 args")
except Exception as e:
    print(f"Error calling with 3 args: {e}")
