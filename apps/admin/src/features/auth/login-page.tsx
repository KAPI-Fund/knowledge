import { useQueryClient } from "@tanstack/react-query";
import { FormEvent, useState } from "react";
import { useNavigate } from "react-router-dom";

import { Button } from "../../components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "../../components/ui/card";
import { Input } from "../../components/ui/input";

import { useLoginMutation } from "./api";
import { setCsrfToken } from "./csrf";

export function LoginPage() {
  const login = useLoginMutation();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [errorMessage, setErrorMessage] = useState("");

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setErrorMessage("");
    try {
      const result = await login.mutateAsync({ username, password });
      setCsrfToken(result.csrfToken);
      // The pre-login /api/auth/me check cached { user: null }. Drop it so the
      // guard refetches the now-authenticated session instead of bouncing back.
      queryClient.removeQueries({ queryKey: ["session"] });
      navigate("/projects");
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : "Sign in failed.");
    }
  }

  return (
    <main className="page">
      <Card className="w-full max-w-md">
        <CardHeader>
          <CardTitle>Sign in</CardTitle>
          <CardDescription>Authenticate to access project operations and system settings.</CardDescription>
        </CardHeader>
        <CardContent>
          <form className="grid gap-4" onSubmit={handleSubmit}>
            <label className="grid gap-2 text-sm font-medium">
              Username
              <Input value={username} onChange={(event) => setUsername(event.target.value)} />
            </label>
            <label className="grid gap-2 text-sm font-medium">
              Password
              <Input
                type="password"
                value={password}
                onChange={(event) => setPassword(event.target.value)}
              />
            </label>
            {errorMessage ? <p className="text-sm text-destructive">{errorMessage}</p> : null}
            <Button disabled={login.isPending} type="submit">
              {login.isPending ? "Signing in..." : "Sign in"}
            </Button>
          </form>
        </CardContent>
      </Card>
    </main>
  );
}
