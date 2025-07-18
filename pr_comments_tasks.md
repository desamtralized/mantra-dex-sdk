# PR Comments Task List

## Task 1: Add markdown extensions to allowed script types
**File:** `src/mcp/server.rs` (lines 4294-4313)  
**Priority:** Medium  
**Description:** Add "md" and "markdown" to the allowed_extensions array so the validation matches the tool description and supports markdown script files.

## Task 2: Add parameter validation to execute_custom_tool method
**File:** `src/mcp/sdk_adapter.rs` (lines 1244-1309)  
**Priority:** High  
**Description:** Add explicit checks for the presence and validity of required parameters before calling each tool-specific method, returning clear and descriptive errors if parameters are missing or invalid.

## Task 3: Add file size validation in parse_file function
**File:** `src/mcp/script_parser.rs` (lines 128-132)  
**Priority:** High  
**Description:** Before reading the file content, add a check to get the file metadata and validate that the file size is within a safe limit. If the file is too large, return an appropriate error instead of reading it.

## Task 4: Enhance validate_script function with comprehensive validation
**File:** `src/mcp/script_parser.rs` (lines 675-692)  
**Priority:** High  
**Description:** Enhance this function by adding checks for:
- Script size limits to prevent DoS attacks
- Parameter values for correctness and allowed formats
- Asset names against expected patterns or allowed lists
- Network names are valid and recognized
- Timeout values fall within acceptable bounds
- Implement these validations with appropriate error returns