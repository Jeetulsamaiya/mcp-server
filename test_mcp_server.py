#!/usr/bin/env python3
"""
Test script for the MCP server implementation.
This script tests all the implemented MCP protocol methods to ensure compatibility
with the MCP Inspector and other MCP clients.
"""

import json
import requests
import sys
from typing import Dict, Any, Optional

class MCPTester:
    def __init__(self, base_url: str = "http://localhost:8080/sse"):
        self.base_url = base_url
        self.request_id = 1
        
    def send_request(self, method: str, params: Optional[Dict[str, Any]] = None) -> Dict[str, Any]:
        """Send a JSON-RPC request to the MCP server."""
        payload = {
            "jsonrpc": "2.0",
            "id": self.request_id,
            "method": method
        }
        
        if params:
            payload["params"] = params
            
        self.request_id += 1
        
        try:
            response = requests.post(
                self.base_url,
                headers={"Content-Type": "application/json"},
                data=json.dumps(payload),
                timeout=10
            )
            response.raise_for_status()
            return response.json()
        except requests.exceptions.RequestException as e:
            return {"error": f"Request failed: {e}"}
    
    def test_initialize(self) -> bool:
        """Test the initialize method."""
        print("Testing initialize...")
        
        params = {
            "protocolVersion": "2025-03-26",
            "capabilities": {
                "roots": {"listChanged": True}
            },
            "clientInfo": {
                "name": "mcp-test-client",
                "version": "1.0.0"
            }
        }
        
        response = self.send_request("initialize", params)
        
        if "error" in response:
            print(f"❌ Initialize failed: {response['error']}")
            return False
            
        if "result" not in response:
            print(f"❌ Initialize failed: No result in response")
            return False
            
        result = response["result"]
        required_fields = ["protocolVersion", "capabilities", "serverInfo"]
        
        for field in required_fields:
            if field not in result:
                print(f"❌ Initialize failed: Missing {field} in result")
                return False
                
        print("✅ Initialize successful")
        print(f"   Server: {result['serverInfo']['name']} v{result['serverInfo']['version']}")
        print(f"   Protocol: {result['protocolVersion']}")
        return True
    
    def test_resources_list(self) -> bool:
        """Test the resources/list method."""
        print("Testing resources/list...")
        
        response = self.send_request("resources/list")
        
        if "error" in response:
            print(f"❌ Resources list failed: {response['error']}")
            return False
            
        if "result" not in response or "resources" not in response["result"]:
            print(f"❌ Resources list failed: No resources in response")
            return False
            
        resources = response["result"]["resources"]
        print(f"✅ Resources list successful: {len(resources)} resources found")
        
        for resource in resources:
            print(f"   - {resource['name']}: {resource['uri']}")
            
        return True
    
    def test_tools_list(self) -> bool:
        """Test the tools/list method."""
        print("Testing tools/list...")
        
        response = self.send_request("tools/list")
        
        if "error" in response:
            print(f"❌ Tools list failed: {response['error']}")
            return False
            
        if "result" not in response or "tools" not in response["result"]:
            print(f"❌ Tools list failed: No tools in response")
            return False
            
        tools = response["result"]["tools"]
        print(f"✅ Tools list successful: {len(tools)} tools found")
        
        for tool in tools:
            print(f"   - {tool['name']}: {tool.get('description', 'No description')}")
            
        return True
    
    def test_prompts_list(self) -> bool:
        """Test the prompts/list method."""
        print("Testing prompts/list...")
        
        response = self.send_request("prompts/list")
        
        if "error" in response:
            print(f"❌ Prompts list failed: {response['error']}")
            return False
            
        if "result" not in response or "prompts" not in response["result"]:
            print(f"❌ Prompts list failed: No prompts in response")
            return False
            
        prompts = response["result"]["prompts"]
        print(f"✅ Prompts list successful: {len(prompts)} prompts found")
        
        for prompt in prompts:
            print(f"   - {prompt['name']}: {prompt.get('description', 'No description')}")
            
        return True
    
    def test_tools_call(self) -> bool:
        """Test the tools/call method."""
        print("Testing tools/call...")
        
        params = {
            "name": "echo",
            "arguments": {
                "text": "Hello from MCP test!"
            }
        }
        
        response = self.send_request("tools/call", params)
        
        if "error" in response:
            print(f"❌ Tool call failed: {response['error']}")
            return False
            
        if "result" not in response:
            print(f"❌ Tool call failed: No result in response")
            return False
            
        result = response["result"]
        if "content" not in result:
            print(f"❌ Tool call failed: No content in result")
            return False
            
        print("✅ Tool call successful")
        print(f"   Result: {result['content']}")
        return True
    
    def test_resources_read(self) -> bool:
        """Test the resources/read method."""
        print("Testing resources/read...")
        
        params = {
            "uri": "text://hello"
        }
        
        response = self.send_request("resources/read", params)
        
        if "error" in response:
            print(f"❌ Resource read failed: {response['error']}")
            return False
            
        if "result" not in response or "contents" not in response["result"]:
            print(f"❌ Resource read failed: No contents in response")
            return False
            
        contents = response["result"]["contents"]
        print(f"✅ Resource read successful: {len(contents)} content items")
        
        for content in contents:
            if content.get("type") == "text":
                print(f"   Text: {content['text'][:50]}...")
                
        return True
    
    def test_prompts_get(self) -> bool:
        """Test the prompts/get method."""
        print("Testing prompts/get...")
        
        params = {
            "name": "greeting",
            "arguments": {
                "name": "MCP Tester"
            }
        }
        
        response = self.send_request("prompts/get", params)
        
        if "error" in response:
            print(f"❌ Prompt get failed: {response['error']}")
            return False
            
        if "result" not in response:
            print(f"❌ Prompt get failed: No result in response")
            return False
            
        result = response["result"]
        if "messages" not in result:
            print(f"❌ Prompt get failed: No messages in result")
            return False
            
        print("✅ Prompt get successful")
        print(f"   Description: {result.get('description', 'No description')}")
        print(f"   Messages: {len(result['messages'])}")
        return True
    
    def run_all_tests(self) -> bool:
        """Run all tests and return True if all pass."""
        print("🚀 Starting MCP Server Tests")
        print("=" * 50)
        
        tests = [
            self.test_initialize,
            self.test_resources_list,
            self.test_tools_list,
            self.test_prompts_list,
            self.test_tools_call,
            self.test_resources_read,
            self.test_prompts_get,
        ]
        
        passed = 0
        total = len(tests)
        
        for test in tests:
            try:
                if test():
                    passed += 1
                print()  # Empty line between tests
            except Exception as e:
                print(f"❌ Test failed with exception: {e}")
                print()
        
        print("=" * 50)
        print(f"📊 Test Results: {passed}/{total} tests passed")
        
        if passed == total:
            print("🎉 All tests passed! MCP server is fully functional.")
            return True
        else:
            print("❌ Some tests failed. Please check the server implementation.")
            return False

if __name__ == "__main__":
    tester = MCPTester()
    success = tester.run_all_tests()
    sys.exit(0 if success else 1)
