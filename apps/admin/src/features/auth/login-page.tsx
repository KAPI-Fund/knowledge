import { FormEvent, useState } from "react";
import { useNavigate } from "react-router-dom";

import { useLoginMutation } from "./api";

export function LoginPage() {
  const login = useLoginMutation();
  const navigate = useNavigate();
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    await login.mutateAsync({ username, password });
    navigate("/projects");
  }

  return (
    <main className="page">
      <form className="card" onSubmit={handleSubmit}>
        <h1>Sign in</h1>
        <label>
          Username
          <input value={username} onChange={(event) => setUsername(event.target.value)} />
        </label>
        <label>
          Password
          <input
            type="password"
            value={password}
            onChange={(event) => setPassword(event.target.value)}
          />
        </label>
        <button type="submit">Sign in</button>
      </form>
    </main>
  );
}
