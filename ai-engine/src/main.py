import sys
import os
from concurrent import futures
from loguru import logger
import grpc

# Add the parent directory of 'src' to the Python path to allow proper package imports
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), "..")))

from src.generated import prism_pb2_grpc
from src.services.ai_service import AiServiceHandler

def serve():
    # Configure logging aesthetics and output
    logger.remove()
    logger.add(
        sys.stderr,
        format="<green>{time:YYYY-MM-DD HH:mm:ss.SSS}</green> | <level>{level: <8}</level> | <cyan>{name}</cyan>:<cyan>{line}</cyan> - <level>{message}</level>",
        level="INFO"
    )

    port = "50051"
    # Initialize the gRPC server with a multi-threaded execution pool
    server = grpc.server(futures.ThreadPoolExecutor(max_workers=10))

    # Register the service handler with the gRPC server
    prism_pb2_grpc.add_AiServiceServicer_to_server(AiServiceHandler(), server)

    # Bind the server to all network interfaces on port 50051
    server.add_insecure_port(f"[::]:{port}")
    logger.info(f"Starting PRISM AI Engine gRPC server on port {port}...")
    server.start()
    logger.info("AI Engine Server successfully started. Listening for requests...")

    try:
        # Keep the main thread alive waiting for incoming connections
        server.wait_for_termination()
    except KeyboardInterrupt:
        logger.info("SIGINT/KeyboardInterrupt detected. Stopping gRPC server...")
        server.stop(grace=0)
        logger.info("AI Engine Server stopped successfully.")

if __name__ == "__main__":
    serve()
