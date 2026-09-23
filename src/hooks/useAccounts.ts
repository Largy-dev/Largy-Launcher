import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { notify } from "@/lib/notify";
import { auth, errorMessage } from "@/services/tauri";
import { useAppStore } from "@/store/appStore";

/** Remembered Microsoft accounts, plus switching and forgetting them. */
export function useAccounts() {
  const queryClient = useQueryClient();
  const account = useAppStore((s) => s.account);
  const setAccount = useAppStore((s) => s.setAccount);
  const accounts = useQuery({ queryKey: ["accounts"], queryFn: auth.listAccounts });
  const refresh = () => queryClient.invalidateQueries({ queryKey: ["accounts"] });

  const switchTo = useMutation({
    mutationFn: (id: string) => auth.switchAccount(id),
    onSuccess: (session) => {
      setAccount(session);
      refresh();
      notify.success({ title: `Compte actif : ${session.profile.name}`, history: false });
    },
    onError: (e) => notify.error({ title: "Changement de compte impossible", message: errorMessage(e) }),
  });

  const forget = useMutation({
    mutationFn: (id: string) => auth.logout(id),
    onSuccess: (_, id) => {
      if (account?.profile.id === id) setAccount(null);
      refresh();
    },
    onError: (e) => notify.error({ title: "Déconnexion impossible", message: errorMessage(e) }),
  });

  return {
    accounts: accounts.data ?? [],
    others: (accounts.data ?? []).filter((a) => a.id !== account?.profile.id),
    switchTo,
    forget,
    refresh,
  };
}
