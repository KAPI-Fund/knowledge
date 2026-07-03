import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import * as api from "../shared/api";
import {
  useActivateConnectionMutation,
  useCreateConnectionMutation,
  useDeleteConnectionMutation,
  useUpdateConnectionMutation,
} from "./queries";

afterEach(() => vi.restoreAllMocks());

function wrapper() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
}

describe("connection mutations", () => {
  it("create calls createProviderConnection", async () => {
    const spy = vi
      .spyOn(api, "createProviderConnection")
      .mockResolvedValue({ providerMode: "x" } as never);
    const { result } = renderHook(() => useCreateConnectionMutation(), { wrapper: wrapper() });
    await act(async () => {
      await result.current.mutateAsync({ label: "L", baseUrl: "u", model: "m" });
    });
    expect(spy).toHaveBeenCalledWith({ label: "L", baseUrl: "u", model: "m" });
  });

  it("update calls updateProviderConnection with id + body", async () => {
    const spy = vi
      .spyOn(api, "updateProviderConnection")
      .mockResolvedValue({ providerMode: "x" } as never);
    const { result } = renderHook(() => useUpdateConnectionMutation(), { wrapper: wrapper() });
    await act(async () => {
      await result.current.mutateAsync({ id: "c1", label: "L", baseUrl: "u", model: "m" });
    });
    expect(spy).toHaveBeenCalledWith("c1", { label: "L", baseUrl: "u", model: "m" });
  });

  it("delete calls deleteProviderConnection", async () => {
    const spy = vi
      .spyOn(api, "deleteProviderConnection")
      .mockResolvedValue({ providerMode: "x" } as never);
    const { result } = renderHook(() => useDeleteConnectionMutation(), { wrapper: wrapper() });
    await act(async () => {
      await result.current.mutateAsync("c1");
    });
    expect(spy).toHaveBeenCalledWith("c1");
  });

  it("activate calls activateProviderConnection", async () => {
    const spy = vi
      .spyOn(api, "activateProviderConnection")
      .mockResolvedValue({ providerMode: "x" } as never);
    const { result } = renderHook(() => useActivateConnectionMutation(), { wrapper: wrapper() });
    await act(async () => {
      await result.current.mutateAsync("c1");
    });
    await waitFor(() => expect(spy).toHaveBeenCalledWith("c1"));
  });
});
