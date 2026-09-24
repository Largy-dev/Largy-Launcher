import { useQuery } from "@tanstack/react-query";

import { serversApi } from "@/services/servers";

/** Live status of a server, shared by every view that shows the same address. */
export function useServerStatus(address: string) {
  return useQuery({
    queryKey: ["server-ping", address],
    queryFn: () => serversApi.ping(address),
    enabled: address.trim() !== "",
    staleTime: 60 * 1000,
    retry: false,
  });
}
