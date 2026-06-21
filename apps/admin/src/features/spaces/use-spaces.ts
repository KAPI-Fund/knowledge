import { useQuery } from "@tanstack/react-query";
import { fetchSpaces } from "../shared/tenancy-api";

export function useSpacesQuery() {
  return useQuery({ queryKey: ["spaces"], queryFn: fetchSpaces });
}
