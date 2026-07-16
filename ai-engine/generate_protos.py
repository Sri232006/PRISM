import os
import subprocess
import sys

def main():
    print("Compiling proto files for Python...")
    # Setup absolute directory paths
    base_dir = os.path.abspath(os.path.dirname(__file__))
    proto_dir = os.path.abspath(os.path.join(base_dir, "..", "proto"))
    output_dir = os.path.abspath(os.path.join(base_dir, "src", "generated"))

    os.makedirs(output_dir, exist_ok=True)

    # Ensure generated directory is treated as a package
    init_file = os.path.join(output_dir, "__init__.py")
    with open(init_file, "a"):
        pass

    proto_file = os.path.join(proto_dir, "prism.proto")

    # Command parameters to run grpc_tools.protoc
    command = [
        sys.executable,
        "-m",
        "grpc_tools.protoc",
        f"-I{proto_dir}",
        f"--python_out={output_dir}",
        f"--grpc_python_out={output_dir}",
        proto_file
    ]

    print(f"Executing: {' '.join(command)}")
    result = subprocess.run(command, capture_output=True, text=True)

    if result.returncode != 0:
        print("Protobuf Compiler Error:")
        print(result.stderr)
        sys.exit(1)

    print("Generated Python proto files successfully.")

    # Fix the generated Python imports to be package-relative (essential for python module resolution)
    grpc_file = os.path.join(output_dir, "prism_pb2_grpc.py")
    if os.path.exists(grpc_file):
        with open(grpc_file, "r") as f:
            content = f.read()

        # Replace standard imports with relative imports
        fixed_content = content.replace(
            "import prism_pb2 as prism__pb2",
            "from . import prism_pb2 as prism__pb2"
        )

        with open(grpc_file, "w") as f:
            f.write(fixed_content)

        print("Successfully patched prism_pb2_grpc.py with package-relative imports.")

if __name__ == "__main__":
    main()
