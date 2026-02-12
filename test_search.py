import sys
import os
from pathlib import Path
import inspect

# Add current dir to sys.path so we can import sttEngine
sys.path.append(os.getcwd())

print(f"CWD: {os.getcwd()}")
print(f"sys.path: {sys.path}")

try:
    from sttEngine.server import search_vectors, BASE_DIR
    print(f"Imported search_vectors from sttEngine.server")
    print(f"search_vectors: {search_vectors}")
    print(f"Type: {type(search_vectors)}")
    
    try:
        sig = inspect.signature(search_vectors)
        print(f"Signature: {sig}")
    except Exception as e:
        print(f"Could not get signature: {e}")

    # Mock content
    content = "This is a test content"
    print(f"Calling search_vectors with content, BASE_DIR ({BASE_DIR}), top_k=6")
    
    try:
        res = search_vectors(content, BASE_DIR, top_k=6)
        print(f"Success. Result type: {type(res)}")
    except TypeError as e:
        print(f"TypeError caught: {e}")
    except Exception as e:
        print(f"Other error caught: {e}")
        import traceback
        traceback.print_exc()

except Exception as e:
    print(f"Setup failed: {e}")
    import traceback
    traceback.print_exc()
