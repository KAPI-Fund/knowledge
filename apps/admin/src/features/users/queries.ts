import { useQuery } from "@tanstack/react-query";

import { listUsers } from "../shared/api";

export function useUsersQuery() {
  return useQuery({
    queryKey: ["users"],
    queryFn: listUsers,
  });
}
