import { useState, type ComponentType, type ReactNode } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "react-router";
import { FileText, FolderOpen, MonitorUp, Pin, PinOff, Play, Settings2, Square, Trash2, Wrench } from "lucide-react";

import { ConfirmDialog } from "@/components/ConfirmDialog";
import { useLaunchInstance } from "@/hooks/useLaunchInstance";
import { notify } from "@/lib/notify";
import { errorMessage, instancesApi, type Instance } from "@/services/tauri";

import { RepairDialog } from "./RepairDialog";

interface ItemProps {
  onSelect?: (event: Event) => void;
  disabled?: boolean;
  variant?: "default" | "destructive";
  title?: string;
  className?: string;
  children: ReactNode;
}

interface MenuParts {
  Item: ComponentType<ItemProps>;
  Separator: ComponentType;
}

/**
 * The quick actions of one instance — the same list behind the ⋯ next to
 * "Jouer" and the sidebar's right-click menu. Render `items` inside a
 * dropdown or a context menu, and `dialogs` anywhere next to it.
 */
export function useInstanceMenu(instance: Instance) {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { play, stop, running } = useLaunchInstance(instance);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [repairing, setRepairing] = useState(false);

  const pin = useMutation({
    mutationFn: () => instancesApi.setPinned(instance.id, !instance.pinned),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["instances"] }),
    onError: (e) => notify.error({ title: "Action impossible", message: errorMessage(e) }),
  });

  const shortcut = useMutation({
    mutationFn: () => instancesApi.createShortcut(instance.id),
    onSuccess: () =>
      notify.success({
        title: "Raccourci créé sur le bureau",
        message: `Double-clique dessus pour lancer ${instance.name} directement.`,
        history: false,
      }),
    onError: (e) => notify.error({ title: "Raccourci impossible", message: errorMessage(e) }),
  });

  const remove = useMutation({
    mutationFn: () => instancesApi.delete(instance.id),
    onSuccess: () => {
      setConfirmDelete(false);
      queryClient.removeQueries({ queryKey: ["instance", instance.id] });
      queryClient.invalidateQueries({ queryKey: ["instances"] });
      notify.success({ title: `${instance.name} supprimée`, history: false });
      navigate("/");
    },
    onError: (e) => notify.error({ title: "Suppression impossible", message: errorMessage(e) }),
  });

  function items({ Item, Separator }: MenuParts, options: { withPlay?: boolean } = {}) {
    return (
      <>
        {options.withPlay &&
          (running ? (
            <Item onSelect={() => stop()}>
              <Square className="fill-current" aria-hidden="true" />
              Arrêter
            </Item>
          ) : (
            <Item onSelect={() => play()}>
              <Play className="fill-current" aria-hidden="true" />
              Jouer
            </Item>
          ))}
        <Item onSelect={() => instancesApi.openFolder(instance.id)}>
          <FolderOpen aria-hidden="true" />
          Ouvrir le dossier
        </Item>
        <Item onSelect={() => instancesApi.openFolder(instance.id, "logs")}>
          <FileText aria-hidden="true" />
          Dossier des logs
        </Item>
        <Item onSelect={() => shortcut.mutate()}>
          <MonitorUp aria-hidden="true" />
          Raccourci sur le bureau
        </Item>
        <Item onSelect={() => pin.mutate()}>
          {instance.pinned ? <PinOff aria-hidden="true" /> : <Pin aria-hidden="true" />}
          {instance.pinned ? "Détacher" : "Épingler en haut"}
        </Item>
        <Item onSelect={() => setRepairing(true)}>
          <Wrench aria-hidden="true" />
          Réparer…
        </Item>
        <Item onSelect={() => navigate(`/instances/${instance.id}?tab=settings`)}>
          <Settings2 aria-hidden="true" />
          Réglages
        </Item>
        <Separator />
        <Item
          variant="destructive"
          disabled={running || instance.protected}
          title={instance.protected ? "Instance protégée — retire la protection dans ses réglages" : undefined}
          onSelect={() => setConfirmDelete(true)}
        >
          <Trash2 aria-hidden="true" />
          Supprimer
        </Item>
      </>
    );
  }

  const dialogs = (
    <>
      <RepairDialog instance={instance} open={repairing} onOpenChange={setRepairing} />
      <ConfirmDialog
        open={confirmDelete}
        onOpenChange={setConfirmDelete}
        title={`Supprimer « ${instance.name} » ?`}
        description="Le dossier de l'instance sera effacé, y compris ses mondes, captures d'écran et configurations. Cette action est définitive."
        confirmLabel="Supprimer définitivement"
        destructive
        pending={remove.isPending}
        onConfirm={() => remove.mutate()}
      />
    </>
  );

  return { items, dialogs };
}
