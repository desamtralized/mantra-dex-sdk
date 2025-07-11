#!/bin/bash

# Mantra DEX MCP Script Runner
# This script compiles the MCP server and executes natural language test scripts

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Function to show usage
show_usage() {
    echo "Usage: $0 [SCRIPT_NAME] [OPTIONS]"
    echo ""
    echo "Arguments:"
    echo "  SCRIPT_NAME    Name of the script to run (without .md extension)"
    echo "                 Available scripts:"
    echo "                   - test_basic_swap"
    echo "                   - test_liquidity_provision"
    echo "                   - test_pool_creation"
    echo "                   - test_complex_scenario"
    echo "                   - test_network_validation"
    echo ""
    echo "Options:"
    echo "  -h, --help                Show this help message"
    echo "  -v, --verbose            Enable verbose output"
    echo "  -d, --debug              Enable debug mode"
    echo "  -t, --timeout SECONDS    Set script execution timeout (default: 300)"
    echo "  -c, --continue-on-fail   Continue execution even if steps fail"
    echo "  --no-validate            Skip outcome validation"
    echo "  --mcp-port PORT          MCP server port (default: 8080)"
    echo "  --transport TYPE         Transport type: stdio or http (default: stdio)"
    echo ""
    echo "Examples:"
    echo "  $0 test_basic_swap"
    echo "  $0 test_complex_scenario --timeout 600 --verbose"
    echo "  $0 test_network_validation --debug --transport http"
}

# Default values
SCRIPT_NAME=""
VERBOSE=false
DEBUG=false
TIMEOUT=300
CONTINUE_ON_FAIL=false
VALIDATE_OUTCOMES=true
MCP_PORT=8080
TRANSPORT="stdio"

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help)
            show_usage
            exit 0
            ;;
        -v|--verbose)
            VERBOSE=true
            shift
            ;;
        -d|--debug)
            DEBUG=true
            VERBOSE=true
            shift
            ;;
        -t|--timeout)
            TIMEOUT="$2"
            shift 2
            ;;
        -c|--continue-on-fail)
            CONTINUE_ON_FAIL=true
            shift
            ;;
        --no-validate)
            VALIDATE_OUTCOMES=false
            shift
            ;;
        --mcp-port)
            MCP_PORT="$2"
            shift 2
            ;;
        --transport)
            TRANSPORT="$2"
            shift 2
            ;;
        -*)
            print_error "Unknown option: $1"
            show_usage
            exit 1
            ;;
        *)
            if [[ -z "$SCRIPT_NAME" ]]; then
                SCRIPT_NAME="$1"
            else
                print_error "Too many arguments"
                show_usage
                exit 1
            fi
            shift
            ;;
    esac
done

# Check if script name is provided
if [[ -z "$SCRIPT_NAME" ]]; then
    print_error "Script name is required"
    show_usage
    exit 1
fi

# Set script path
SCRIPT_PATH="scripts/${SCRIPT_NAME}.md"

# Check if script file exists
if [[ ! -f "$SCRIPT_PATH" ]]; then
    print_error "Script file not found: $SCRIPT_PATH"
    echo ""
    echo "Available scripts:"
    for script in scripts/*.md; do
        if [[ -f "$script" ]]; then
            basename "$script" .md
        fi
    done
    exit 1
fi

print_status "Starting Mantra DEX MCP Script Runner"
print_status "Script: $SCRIPT_NAME"
print_status "Transport: $TRANSPORT"
print_status "Timeout: ${TIMEOUT}s"

# Step 1: Build the MCP server
print_status "Building MCP server..."
if $VERBOSE; then
    cargo build --features mcp --bin mcp-server
else
    cargo build --features mcp --bin mcp-server > /dev/null 2>&1
fi

if [[ $? -ne 0 ]]; then
    print_error "Failed to build MCP server"
    exit 1
fi

print_success "MCP server built successfully"

# Step 2: Start MCP server in background
print_status "Starting MCP server..."

# Prepare MCP server arguments
MCP_ARGS=""
if $DEBUG; then
    MCP_ARGS="$MCP_ARGS --debug"
fi

if [[ "$TRANSPORT" == "http" ]]; then
    MCP_ARGS="$MCP_ARGS --transport http --port $MCP_PORT"
else
    MCP_ARGS="$MCP_ARGS --transport stdio"
fi

# Start MCP server
if [[ "$TRANSPORT" == "http" ]]; then
    print_status "Starting MCP server on HTTP port $MCP_PORT..."
    cargo run --bin mcp-server --features mcp -- $MCP_ARGS > mcp_server.log 2>&1 &
    MCP_PID=$!
    
    # Wait for server to start
    sleep 3
    
    # Check if server is running
    if ! kill -0 $MCP_PID 2>/dev/null; then
        print_error "Failed to start MCP server"
        cat mcp_server.log
        exit 1
    fi
    
    print_success "MCP server started (PID: $MCP_PID)"
else
    print_status "MCP server will be started in stdio mode when script execution begins"
fi

# Step 3: Execute the script
print_status "Executing script: $SCRIPT_NAME"

# Create a temporary Python script to call the MCP server
cat > temp_script_runner.py << 'EOF'
import json
import sys
import subprocess
import time
import signal
import os

def run_script_via_mcp(script_path, config):
    """Run a script via MCP server"""
    
    # Read the script content
    with open(script_path, 'r') as f:
        script_content = f.read()
    
    # Prepare MCP request
    request = {
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "run_script",
            "arguments": {
                "script_content": script_content,
                "config": config
            }
        }
    }
    
    # Start MCP server process
    server_cmd = ["cargo", "run", "--bin", "mcp-server", "--features", "mcp", "--", "--transport", "stdio"]
    
    if config.get("debug", False):
        server_cmd.append("--debug")
    
    print(f"Starting MCP server with command: {' '.join(server_cmd)}")
    
    try:
        proc = subprocess.Popen(
            server_cmd,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True
        )
        
        # Send request
        request_json = json.dumps(request)
        print(f"Sending request: {request_json}")
        
        stdout, stderr = proc.communicate(input=request_json + "\n", timeout=config.get("timeout", 300))
        
        if proc.returncode != 0:
            print(f"MCP server failed with return code: {proc.returncode}")
            print(f"STDERR: {stderr}")
            return False
        
        print(f"MCP server output: {stdout}")
        
        # Parse response
        try:
            response = json.loads(stdout.strip())
            
            if "error" in response:
                print(f"MCP error: {response['error']}")
                return False
            
            if "result" in response:
                result = response["result"]
                print(f"Script execution result: {json.dumps(result, indent=2)}")
                
                # Check if script succeeded
                if result.get("status") == "Success":
                    print("Script executed successfully!")
                    return True
                else:
                    print(f"Script failed with status: {result.get('status')}")
                    if "error" in result:
                        print(f"Error: {result['error']}")
                    return False
            
        except json.JSONDecodeError as e:
            print(f"Failed to parse MCP response: {e}")
            print(f"Raw response: {stdout}")
            return False
        
    except subprocess.TimeoutExpired:
        print("Script execution timed out")
        proc.kill()
        return False
    except Exception as e:
        print(f"Error executing script: {e}")
        return False

# Main execution
if __name__ == "__main__":
    import sys
    
    if len(sys.argv) < 2:
        print("Usage: python temp_script_runner.py <script_path> [config_json]")
        sys.exit(1)
    
    script_path = sys.argv[1]
    config_json = sys.argv[2] if len(sys.argv) > 2 else "{}"
    
    try:
        config = json.loads(config_json)
    except json.JSONDecodeError:
        print("Invalid config JSON")
        sys.exit(1)
    
    success = run_script_via_mcp(script_path, config)
    sys.exit(0 if success else 1)
EOF

# Prepare configuration
CONFIG_JSON=$(cat << EOF
{
    "max_script_timeout": $TIMEOUT,
    "default_step_timeout": 30,
    "continue_on_failure": $CONTINUE_ON_FAIL,
    "validate_outcomes": $VALIDATE_OUTCOMES,
    "debug": $DEBUG
}
EOF
)

# Execute the script
print_status "Running script with configuration: $CONFIG_JSON"

if python3 temp_script_runner.py "$SCRIPT_PATH" "$CONFIG_JSON"; then
    print_success "Script executed successfully!"
    EXIT_CODE=0
else
    print_error "Script execution failed"
    EXIT_CODE=1
fi

# Cleanup
print_status "Cleaning up..."

# Kill MCP server if running
if [[ -n "$MCP_PID" ]]; then
    if kill -0 $MCP_PID 2>/dev/null; then
        print_status "Stopping MCP server (PID: $MCP_PID)"
        kill $MCP_PID
        sleep 2
        if kill -0 $MCP_PID 2>/dev/null; then
            print_warning "Force killing MCP server"
            kill -9 $MCP_PID
        fi
    fi
fi

# Remove temporary files
rm -f temp_script_runner.py mcp_server.log

print_success "Cleanup completed"
exit $EXIT_CODE