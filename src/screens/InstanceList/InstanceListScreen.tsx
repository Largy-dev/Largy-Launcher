import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { LayoutGrid, Loader2, Plus } from "lucide-react";

import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/EmptyState";
import { PageHeader } from "@/components/PageHeader";
import { instancesApi } from "@/services/tauri";

import { CreateInstanceDialog } from "./CreateInstanceDialog";
import { InstanceCard } from "./InstanceCard";

export function InstanceListScreen() {
  const [createOpen, setCreateOpen] = useState(false);
  const { data: instances, isLoading } = useQuery({
    queryKey: ["instances"],
    queryFn: instancesApi.list,
  });

  return (
    <div className="flex flex-1 flex-col">
      <PageHeader
        title="Instances"
        description="Tes installations Minecraft, vanilla ou modées."
        action={
          <Button onClick={() => setCreateOpen(true)} className="gap-1.5">
            <Plus className="size-4" aria-hidden="true" />
            Nouvelle instance
          </Button>
        }
      />

      {isLoading ? (
        <div className="flex flex-1 items-center justify-center gap-2 text-sm text-muted-foreground">
          <Loader2 className="size-4 animate-spin" aria-hidden="true" />
          Chargement…
        </div>
      ) : !instances || instances.length === 0 ? (
        <EmptyState
          icon={LayoutGrid}
          title="Aucune instance pour le moment"
          description="Crée une instance vanilla ou installe un modpack pour commencer à jouer."
          action={<Button onClick={() => setCreateOpen(true)}>Nouvelle instance</Button>}
        />
      ) : (
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {instances.map((instance) => (
            <InstanceCard key={instance.id} instance={instance} />
          ))}
        </div>
      )}

      <CreateInstanceDialog open={createOpen} onOpenChange={setCreateOpen} />
    </div>
  );
}
