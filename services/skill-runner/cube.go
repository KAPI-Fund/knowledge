package main

import (
	"context"

	cubesandbox "github.com/tencentcloud/CubeSandbox/sdk/go"
)

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
	sb, err := f.client.Create(ctx, cubesandbox.CreateOptions{TemplateID: f.templateID})
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

func (c *cubeSandbox) RunCommand(ctx context.Context, cmd string, env map[string]string) (CommandResult, error) {
	res, err := c.sb.Commands().Run(ctx, cmd, cubesandbox.CommandOptions{Envs: env})
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
