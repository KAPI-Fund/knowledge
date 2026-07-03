import { useState } from "react";

import { PageHeader } from "@/components/shared/page-header";
import { Card, CardContent } from "@/components/ui/card";

import { DefaultsSection } from "./sections/defaults-section";
import { EmbeddingSection } from "./sections/embedding-section";
import { ImageSection } from "./sections/image-section";
import { LlmConnectionsSection } from "./sections/llm-connections-section";
import { WebSearchSection } from "./sections/web-search-section";
import { SettingsNav, type SettingsSectionId } from "./settings-nav";

export function SettingsPage() {
  const [section, setSection] = useState<SettingsSectionId>("llm");

  return (
    <div className="grid gap-6">
      <PageHeader
        description="Configure model providers, embeddings, image generation, and web search."
        title="Settings"
      />
      <div className="grid gap-6 md:grid-cols-[200px_1fr]">
        <Card className="h-fit">
          <CardContent className="p-2">
            <SettingsNav active={section} onSelect={setSection} />
          </CardContent>
        </Card>
        <div className="min-w-0">
          {section === "llm" ? <LlmConnectionsSection /> : null}
          {section === "embedding" ? <EmbeddingSection /> : null}
          {section === "image" ? <ImageSection /> : null}
          {section === "search" ? <WebSearchSection /> : null}
          {section === "defaults" ? <DefaultsSection /> : null}
        </div>
      </div>
    </div>
  );
}
