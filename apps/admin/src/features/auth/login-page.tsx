import { zodResolver } from "@hookform/resolvers/zod";
import { useQueryClient } from "@tanstack/react-query";
import { LibraryBig, Loader2 } from "lucide-react";
import { useState } from "react";
import { useForm } from "react-hook-form";
import { Link, useNavigate } from "react-router-dom";
import { z } from "zod";

import { Button } from "@/components/ui/button";
import {
  Form,
  FormControl,
  FormField,
  FormItem,
  FormLabel,
  FormMessage,
} from "@/components/ui/form";
import { Input } from "@/components/ui/input";

import { AuthLayout } from "./auth-layout";
import { useLoginMutation } from "./api";
import { setCsrfToken } from "./csrf";

const loginSchema = z.object({
  username: z.string().min(1, "Username is required."),
  password: z.string().min(1, "Password is required."),
});

type LoginValues = z.infer<typeof loginSchema>;

export function LoginPage() {
  const login = useLoginMutation();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [errorMessage, setErrorMessage] = useState("");

  const form = useForm<LoginValues>({
    resolver: zodResolver(loginSchema),
    defaultValues: { username: "", password: "" },
  });

  async function onSubmit(values: LoginValues) {
    setErrorMessage("");
    try {
      const result = await login.mutateAsync(values);
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
    <AuthLayout>
      <div className="grid gap-2 text-center">
        <div className="mx-auto mb-2 flex size-10 items-center justify-center rounded-lg bg-primary text-primary-foreground lg:hidden">
          <LibraryBig className="size-5" />
        </div>
        <h1 className="text-2xl font-semibold tracking-tight">Sign in</h1>
        <p className="text-sm text-muted-foreground">
          Authenticate to access project operations and system settings.
        </p>
      </div>
      <Form {...form}>
        <form className="grid gap-4" onSubmit={form.handleSubmit(onSubmit)}>
          <FormField
            control={form.control}
            name="username"
            render={({ field }) => (
              <FormItem>
                <FormLabel>Username</FormLabel>
                <FormControl>
                  <Input autoComplete="username" {...field} />
                </FormControl>
                <FormMessage />
              </FormItem>
            )}
          />
          <FormField
            control={form.control}
            name="password"
            render={({ field }) => (
              <FormItem>
                <FormLabel>Password</FormLabel>
                <FormControl>
                  <Input autoComplete="current-password" type="password" {...field} />
                </FormControl>
                <FormMessage />
              </FormItem>
            )}
          />
          {errorMessage ? <p className="text-sm text-destructive">{errorMessage}</p> : null}
          <Button disabled={login.isPending} type="submit">
            {login.isPending ? <Loader2 className="animate-spin" /> : null}
            {login.isPending ? "Signing in..." : "Sign in"}
          </Button>
        </form>
      </Form>
      <p className="text-center text-sm text-muted-foreground">
        Don't have an account?{" "}
        <Link
          className="font-medium text-foreground underline-offset-4 hover:underline"
          to="/register"
        >
          Create one
        </Link>
      </p>
    </AuthLayout>
  );
}
