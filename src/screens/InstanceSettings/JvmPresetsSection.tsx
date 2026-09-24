import { useState } from "react";
import { Check, Save, Trash2 } from "lucide-react";

import { SettingRow, SettingSection } from "@/components/settings/SettingsKit";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { useJvmPresets } from "@/hooks/useJvmPresets";

interface JvmPresetsSectionProps {
  minMb: number | null;
  maxMb: number;
  args: string[];
  onApply: (preset: { minMb: number; maxMb: number; jvmArgs: string }) => void;
}

/** Named, reusable memory/JVM combinations saved from this instance and applicable to any other. */
export function JvmPresetsSection({ minMb, maxMb, args, onApply }: JvmPresetsSectionProps) {
  const { presets, addPreset, removePreset, saving } = useJvmPresets();
  const [name, setName] = useState("");
  const [selected, setSelected] = useState("");

  return (
    <SettingSection title="Presets personnalisés">
      {presets.length > 0 && (
        <SettingRow
          label="Appliquer un preset"
          description="Remplace la mémoire et les arguments JVM de cette instance — pense à Enregistrer ensuite."
          control={
            <div className="flex items-center gap-2">
              <Select value={selected} onValueChange={setSelected}>
                <SelectTrigger size="sm" className="w-48">
                  <SelectValue placeholder="Choisir un preset" />
                </SelectTrigger>
                <SelectContent>
                  {presets.map((p) => (
                    <SelectItem key={p.name} value={p.name}>
                      {p.name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              <Button
                size="sm"
                variant="outline"
                className="gap-1.5"
                disabled={!selected}
                onClick={() => {
                  const preset = presets.find((p) => p.name === selected);
                  if (preset)
                    onApply({
                      minMb: preset.min_memory_mb,
                      maxMb: preset.max_memory_mb,
                      jvmArgs: preset.extra_jvm_args.join(" "),
                    });
                }}
              >
                <Check aria-hidden="true" />
                Appliquer
              </Button>
              <Button
                size="icon-sm"
                variant="ghost"
                title="Supprimer ce preset"
                disabled={!selected || saving}
                onClick={() => {
                  removePreset(selected);
                  setSelected("");
                }}
              >
                <Trash2 aria-hidden="true" />
              </Button>
            </div>
          }
        />
      )}
      <SettingRow
        label="Enregistrer les réglages actuels"
        description="Sauvegarde la mémoire et les arguments JVM de cette instance sous un nom, pour les réutiliser ailleurs."
        control={
          <div className="flex items-center gap-2">
            <Input
              className="w-40"
              placeholder="Nom du preset"
              maxLength={40}
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
            <Button
              size="sm"
              variant="outline"
              className="gap-1.5"
              disabled={!name.trim() || saving}
              onClick={() => {
                addPreset({ name: name.trim(), min_memory_mb: minMb ?? 0, max_memory_mb: maxMb, extra_jvm_args: args });
                setName("");
              }}
            >
              <Save aria-hidden="true" />
              Enregistrer
            </Button>
          </div>
        }
      />
    </SettingSection>
  );
}
