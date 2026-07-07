package main

import (
	"context"
	"fmt"
)

// CommandResult 是一次沙箱内命令执行的结果。
type CommandResult struct {
	ExitCode int
	Stdout   string
	Stderr   string
}

// Sandbox 是渲染流程需要的最小沙箱能力(便于 fake 测试;真实现包官方 SDK)。
type Sandbox interface {
	WriteFile(ctx context.Context, path, content string) error
	RunCommand(ctx context.Context, cmd string, env map[string]string) (CommandResult, error)
	ReadFile(ctx context.Context, path string) (string, error)
	Kill(ctx context.Context) error
}

// SandboxFactory 创建一个新沙箱(每次 render 一个)。
type SandboxFactory interface {
	Create(ctx context.Context) (Sandbox, error)
}

// --- 测试用 fake ---

type fakeSandbox struct {
	writes       map[string]string
	files        map[string]string // ReadFile 的返回内容
	runResult    CommandResult
	runErr       error
	killed       bool
	failReadPath string
}

func (f *fakeSandbox) WriteFile(_ context.Context, path, content string) error {
	if f.writes == nil {
		f.writes = map[string]string{}
	}
	f.writes[path] = content
	return nil
}

func (f *fakeSandbox) RunCommand(_ context.Context, _ string, _ map[string]string) (CommandResult, error) {
	return f.runResult, f.runErr
}

func (f *fakeSandbox) ReadFile(_ context.Context, path string) (string, error) {
	if path == f.failReadPath {
		return "", fmt.Errorf("no such file: %s", path)
	}
	if v, ok := f.files[path]; ok {
		return v, nil
	}
	return "", fmt.Errorf("no such file: %s", path)
}

func (f *fakeSandbox) Kill(_ context.Context) error {
	f.killed = true
	return nil
}

type fakeFactory struct {
	sb        *fakeSandbox
	createErr error
}

func (f *fakeFactory) Create(_ context.Context) (Sandbox, error) {
	if f.createErr != nil {
		return nil, f.createErr
	}
	return f.sb, nil
}
