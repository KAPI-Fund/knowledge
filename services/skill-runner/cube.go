package main

import (
	"context"
	"strings"
	"time"

	cubesandbox "github.com/tencentcloud/CubeSandbox/sdk/go"
)

// sandboxTTL 是微 VM 的存活上限,必须大于 renderTimeout,否则 CubeMaster 会在
// codex 还在跑时按默认 300s TTL 杀掉沙箱,流式连接被切断报 "unexpected EOF"。
const sandboxTTL = renderTimeout + 60*time.Second

// cubeFactory 用官方 SDK 每次 Create 一个真实微 VM。
type cubeFactory struct {
	client     *cubesandbox.Client
	templateID string
}

func newCubeFactory(templateID string) (SandboxFactory, error) {
	// CUBE_API_URL / CUBE_API_KEY(或 E2B_ 变量)从环境读。
	client := cubesandbox.NewClient(cubesandbox.NewConfigFromEnv())
	return &cubeFactory{client: client, templateID: templateID}, nil
}

func (f *cubeFactory) Create(ctx context.Context) (Sandbox, error) {
	sb, err := f.client.Create(ctx, cubesandbox.CreateOptions{TemplateID: f.templateID, Timeout: sandboxTTL})
	if err != nil {
		return nil, err
	}
	return &cubeSandbox{sb: sb}, nil
}

type cubeSandbox struct {
	sb *cubesandbox.Sandbox
}

func (c *cubeSandbox) WriteFile(ctx context.Context, path, content string) error {
	return c.sb.Files().Write(ctx, path, []byte(content))
}

// lineSplitter 把 SDK 按 Data 事件到达的文本(可能半行)缓冲并按整行切出。
func lineSplitter(onLine func(string)) func(cubesandbox.OutputMessage) {
	var pending string
	return func(m cubesandbox.OutputMessage) {
		pending += m.Text
		for {
			i := strings.IndexByte(pending, '\n')
			if i < 0 {
				break
			}
			line := strings.TrimRight(pending[:i], "\r")
			pending = pending[i+1:]
			if strings.TrimSpace(line) != "" {
				onLine(line)
			}
		}
	}
}

func (c *cubeSandbox) RunCommand(ctx context.Context, cmd string, env map[string]string, onOutput func(string)) (CommandResult, error) {
	opts := cubesandbox.CommandOptions{Envs: env}
	if onOutput != nil {
		// codex exec 的过程日志(thinking/工具调用)走 stderr,最终消息才走 stdout,
		// 两路都接到同一回调;各自独立缓冲,互不串行内容。
		opts.OnStdout = lineSplitter(onOutput)
		opts.OnStderr = lineSplitter(onOutput)
	}
	res, err := c.sb.Commands().Run(ctx, cmd, opts)
	if err != nil {
		return CommandResult{}, err
	}
	return CommandResult{ExitCode: res.ExitCode, Stdout: res.Stdout, Stderr: res.Stderr}, nil
}

func (c *cubeSandbox) ReadFile(ctx context.Context, path string) (string, error) {
	return c.sb.Files().Read(ctx, path)
}

func (c *cubeSandbox) Kill(ctx context.Context) error {
	return c.sb.Kill(ctx)
}
