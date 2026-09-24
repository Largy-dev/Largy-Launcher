import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ClipboardCopy, History, Save } from "lucide-react";

import { SettingRow, SettingSection } from "@/components/settings/SettingsKit";
import { Button } from "@/components/ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import { formatDuration, formatRelative } from "@/lib/format";
import { notify } from "@/lib/notify";
import { errorMessage, instancesApi, type Instance } from "@/services/tauri";

const MAX_NOTES_LEN = 4000;

/** Pin/protect toggles, a free-text note, copying settings from another instance, and recent session history. */
export function OrganizationSection({ instance }: { instance: Instance }) {
  const queryClient = useQueryClient();
  const invalidate = () => queryClient.invalidateQueries({ queryKey: ["instance", instance.id] });

  const pin = useMutation({
    mutationFn: (pinned: boolean) => instancesApi.setPinned(instance.id, pinned),
    onSuccess: invalidate,
    onError: (e) => notify.error({ title: "Action impossible", message: errorMessage(e) }),
  });
  const protect = useMutation({
    mutationFn: (protected_: boolean) => instancesApi.setProtected(instance.id, protected_),
    onSuccess: invalidate,
    onError: (e) => notify.error({ title: "Action impossible", message: errorMessage(e) }),
  });

  const [notes, setNotes] = useState(instance.notes);
  const notesDirty = notes !== instance.notes;
  const saveNotes = useMutation({
    mutationFn: () => instancesApi.setNotes(instance.id, notes),
    onSuccess: () => {
      invalidate();
      notify.success({ title: "Notes enregistrées", history: false });
    },
    onError: (e) => notify.error({ title: "Enregistrement impossible", message: errorMessage(e) }),
  });

  const { data: instances } = useQuery({ queryKey: ["instances"], queryFn: instancesApi.list });
  const others = (instances ?? []).filter((i) => i.id !== instance.id);
  const [sourceId, setSourceId] = useState<string>("");
  const copySettings = useMutation({
    mutationFn: () => instancesApi.copySettings(sourceId, instance.id),
    onSuccess: (updated) => {
      queryClient.setQueryData(["instance", instance.id], updated);
      notify.success({ title: "Réglages copiés", message: "Mémoire, JVM, fenêtre et serveur ont été repris." });
    },
    onError: (e) => notify.error({ title: "Copie impossible", message: errorMessage(e) }),
  });

  return (
    <>
      <SettingSection title="Organisation">
        <SettingRow
          label="Épinglée"
          description="Toujours en haut de la liste des instances, quel que soit le tri."
          control={<Switch checked={instance.pinned} onCheckedChange={(v) => pin.mutate(v)} disabled={pin.isPending} />}
        />
        <SettingRow
          label="Protégée contre la suppression"
          description="Le bouton Supprimer restera bloqué tant que c'est activé."
          control={
            <Switch
              checked={instance.protected}
              onCheckedChange={(v) => protect.mutate(v)}
              disabled={protect.isPending}
            />
          }
        />
        {others.length > 0 && (
          <SettingRow
            label="Copier les réglages depuis…"
            description="Mémoire, arguments JVM, Java, fenêtre et serveur auto-rejoint d'une autre instance."
            control={
              <div className="flex items-center gap-2">
                <Select value={sourceId} onValueChange={setSourceId}>
                  <SelectTrigger size="sm" className="w-48">
                    <SelectValue placeholder="Choisir une instance" />
                  </SelectTrigger>
                  <SelectContent>
                    {others.map((i) => (
                      <SelectItem key={i.id} value={i.id}>
                        {i.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
                <Button
                  size="sm"
                  variant="outline"
                  className="gap-1.5"
                  disabled={!sourceId || copySettings.isPending}
                  onClick={() => copySettings.mutate()}
                >
                  <ClipboardCopy aria-hidden="true" />
                  Copier
                </Button>
              </div>
            }
          />
        )}
      </SettingSection>

      <SettingSection title="Notes">
        <div className="p-4">
          <Textarea
            className="min-h-24"
            placeholder="Réglages particuliers, mods à surveiller, mot de passe du serveur…"
            maxLength={MAX_NOTES_LEN}
            value={notes}
            onChange={(e) => setNotes(e.target.value)}
          />
          <div className="mt-2 flex justify-end">
            <Button
              size="sm"
              variant="outline"
              className="gap-1.5"
              disabled={!notesDirty || saveNotes.isPending}
              onClick={() => saveNotes.mutate()}
            >
              <Save aria-hidden="true" />
              Enregistrer les notes
            </Button>
          </div>
        </div>
      </SettingSection>

      {instance.sessions.length > 0 && (
        <SettingSection title="Sessions récentes">
          <div className="divide-y divide-border/60">
            {instance.sessions.slice(0, 5).map((session) => (
              <div key={session.started_at} className="flex items-center gap-3 px-4 py-2.5 text-sm">
                <History className="size-3.5 shrink-0 text-muted-foreground" aria-hidden="true" />
                <span className="flex-1 text-muted-foreground">{formatRelative(session.started_at)}</span>
                <span className="font-medium tabular-nums">{formatDuration(session.duration_seconds)}</span>
              </div>
            ))}
          </div>
        </SettingSection>
      )}
    </>
  );
}
