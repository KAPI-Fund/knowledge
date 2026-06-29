import { useQueryClient } from "@tanstack/react-query";
import { FormEvent, useState } from "react";
import { Link, useNavigate } from "react-router-dom";

import { Button } from "../../components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "../../components/ui/card";
import { Input } from "../../components/ui/input";

import { useRegisterMutation } from "./api";
import { setCsrfToken } from "./csrf";

export function RegisterPage() {
  const register = useRegisterMutation();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [errorMessage, setErrorMessage] = useState("");

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setErrorMessage("");
    if (password.length < 8) {
      setErrorMessage("Password must be at least 8 characters.");
      return;
    }
    if (password !== confirmPassword) {
      setErrorMessage("Passwords do not match.");
      return;
    }
    try {
      const result = await register.mutateAsync({ username, password });
      setCsrfToken(result.csrfToken);
      // Registration logs the user in; drop any cached anonymous session so the
      // guard reads the fresh authenticated state instead of bouncing to /login.
      queryClient.removeQueries({ queryKey: ["session"] });
      navigate("/projects");
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : "Sign up failed.");
    }
  }

  return (
    <main className="page">
      <Card className="w-full max-w-md">
        <CardHeader>
          <CardTitle>Create account</CardTitle>
          <CardDescription>
            Register to access project operations and system settings.
          </CardDescription>
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
            <label className="grid gap-2 text-sm font-medium">
              Confirm password
              <Input
                type="password"
                value={confirmPassword}
                onChange={(event) => setConfirmPassword(event.target.value)}
              />
            </label>
            {errorMessage ? <p className="text-sm text-destructive">{errorMessage}</p> : null}
            <Button disabled={register.isPending} type="submit">
              {register.isPending ? "Creating account..." : "Create account"}
            </Button>
            <p className="text-sm text-muted-foreground">
              Already have an account?{" "}
              <Link
                className="font-medium text-foreground underline-offset-4 hover:underline"
                to="/login"
              >
                Sign in
              </Link>
            </p>
          </form>
        </CardContent>
      </Card>
    </main>
  );
}
