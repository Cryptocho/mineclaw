const { Server } = require("@modelcontextprotocol/sdk/server/index.js");
const { StdioServerTransport } = require("@modelcontextprotocol/sdk/server/stdio.js");
const { CallToolRequestSchema, ListToolsRequestSchema, InitializeRequestSchema } = require("@modelcontextprotocol/sdk/types.js");
const { exec } = require("child_process");
const minimist = require("minimist");
const os = require("os");

// 解析命令行参数
const args = minimist(process.argv.slice(2));
const shellPath = args.shell || undefined; // 如果未指定，exec 会使用默认值 (Windows: cmd.exe, Unix: /bin/sh)

// 探测 Shell 类型
function detectShellType() {
    if (shellPath) {
        const lowerPath = shellPath.toLowerCase();
        if (lowerPath.includes("powershell") || lowerPath.includes("pwsh")) return "powershell";
        if (lowerPath.includes("bash")) return "bash";
        if (lowerPath.includes("zsh")) return "zsh";
        if (lowerPath.includes("cmd.exe")) return "cmd";
    }
    
    // 如果没有指定，探测默认环境
    if (os.platform() === "win32") {
        return "cmd"; // Node.js exec 默认在 Windows 上使用 cmd.exe
    } else {
        return "bash"; // Unix 默认通常是 sh/bash
    }
}

const shellType = detectShellType();
console.error(`[Terminal MCP] Detected shell type: ${shellType}`);

const server = new Server(
  {
    name: "terminal-server",
    version: "1.0.0",
  },
  {
    capabilities: {
      tools: {},
    },
  }
);

// 我们需要在初始化时返回 Shell 信息，但 MCP SDK 封装了 InitializeRequest
// 我们可以通过 ListTools 的 description 或者自定义协议扩展来传递
// 但最简单的方法是提供一个 `get_shell_info` 工具，让 Client 首次连接时调用

server.setRequestHandler(ListToolsRequestSchema, async (request) => {
  return {
    tools: [
      {
        name: "execute_command",
        description: `Execute a command in the terminal (Current Shell: ${shellType})`,
        inputSchema: {
          type: "object",
          properties: {
            command: {
              type: "string",
              description: "The command to execute",
            },
          },
          required: ["command"],
        },
      },
      {
        name: "get_shell_info",
        description: "Get information about the current shell environment",
        inputSchema: {
          type: "object",
          properties: {},
        },
      },
    ],
  };
});

server.setRequestHandler(CallToolRequestSchema, async (request) => {
  if (request.params.name === "execute_command") {
    const command = request.params.arguments.command;
    return new Promise((resolve) => {
      // 使用配置的 shell
      const options = shellPath ? { shell: shellPath } : {};
      
      exec(command, options, (error, stdout, stderr) => {
        if (error) {
          resolve({
            content: [
              {
                type: "text",
                text: `Error: ${error.message}\nStderr: ${stderr}`,
              },
            ],
            isError: true,
          });
        } else {
          resolve({
            content: [
              {
                type: "text",
                text: stdout || stderr || "Command executed successfully with no output",
              },
            ],
          });
        }
      });
    });
  } else if (request.params.name === "get_shell_info") {
      return {
          content: [
              {
                  type: "text",
                  text: JSON.stringify({
                      os: os.platform(),
                      shell_path: shellPath || "default",
                      shell_type: shellType,
                  }, null, 2)
              }
          ]
      };
  }
  
  throw new Error("Tool not found");
});

async function run() {
  const transport = new StdioServerTransport();
  await server.connect(transport);
  if (shellPath) {
    console.error(`[Terminal MCP] Running with shell: ${shellPath}`);
  } else {
    console.error(`[Terminal MCP] Running with default system shell`);
  }
}

run().catch((error) => {
  console.error(error);
  process.exit(1);
});
