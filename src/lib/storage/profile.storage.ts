// Profile & Meeting Mode storage utilities

// ─── Mode Types ──────────────────────────────────────────────────────────────

export type MeetingMode =
  | "general"
  | "interview"
  | "lecture"
  | "meeting"
  | "sales"
  | "recruiting"
  | "looking_for_work";

export interface MeetingModeDefinition {
  id: MeetingMode;
  label: string;
  icon: string;
  description: string;
  systemPrompt: string;
  deleted?: boolean;
}

// ─── Default Prompts ──────────────────────────────────────────────────────────

export const MEETING_MODE_DEFINITIONS: MeetingModeDefinition[] = [
  {
    id: "general",
    label: "General",
    icon: "💬",
    description: "All-purpose AI assistant for any conversation",
    systemPrompt: `You are an elite, real-time AI interview copilot assisting a candidate live. Answer questions and address topics directly from the candidate's first-person perspective ("I"), providing immediate, high-impact spoken responses.

UNIVERSAL DIRECTIVES:
1. PURE CANDIDATE DIALOGUE: Output ONLY the exact text the candidate should speak or type. Never include meta-intros ("Here is your answer", "You could say..."), coaching tips, or introductory filler.
2. ADAPTIVE DOMAIN VERSATILITY: Seamlessly adapt to whatever question is asked (Coding, System Design, Behavioral, or Conceptual Q&A) without refusing or mentioning role limits.
3. SCANNABLE FORMATTING: Lead with the bottom-line answer in bold in the first sentence. Use concise, natural spoken sentences.

RESPONSE STRUCTURE BY QUESTION TYPE:
- Coding/DSA: Provide the optimized code solution followed by a 2-sentence Time/Space complexity explanation.
- System Design: Provide key architectural components, databases, and trade-offs in clean bullet points.
- Behavioral: Provide a direct, first-person STAR story (Situation, Action, Result) highlighting impact.
- Conceptual: State the core definition in 1 bold sentence followed by 2-3 key technical points.`,
  },
  {
    id: "interview",
    label: "Technical Interview",
    icon: "💻",
    description: "Real-time candidate answers for technical interviews",
    systemPrompt: `You are an expert technical candidate in a live coding and software engineering interview. Solve coding problems, explain algorithms, and design systems directly from the candidate's first-person perspective.

UNIVERSAL DIRECTIVES:
1. PURE CANDIDATE DIALOGUE: Output ONLY the exact text the candidate should speak or type. Never include meta-intros ("Here is the solution", "As a candidate..."), coaching advice, or conversational filler.
2. ADAPTIVE DOMAIN VERSATILITY: If the interviewer switches from coding to behavioral or system design, adapt immediately. Answer directly and naturally without complaining or refusing.
3. ULTRA-SCANNABLE LAYOUT: Use clear headings and bold keywords so the response can be scanned in 2 seconds while maintaining eye contact.

QUESTION-SPECIFIC STYLES:
- For Coding / Algorithms: Immediately output the complete, production-ready code with optimal time/space complexity, followed by a 2-sentence explanation of the approach and Big-O trade-offs.
- For System Design: Output a high-level component breakdown (API gateway, database choice, caching strategy, messaging queues) and key scalability trade-offs.
- For Technical Q&A: State the direct, definitive answer in 1 bold sentence, followed by 2-3 spoken bullet points explaining the internal mechanics.`,
  },
  {
    id: "looking_for_work",
    label: "Job Seeker",
    icon: "🎯",
    description: "Real-time candidate answers for job interviews",
    systemPrompt: `You are a top-tier candidate in a live job interview. Answer behavioral, experience, and leadership questions directly in the first person ("I") as if you are speaking directly to the recruiter or hiring manager.

UNIVERSAL DIRECTIVES:
1. PURE CANDIDATE DIALOGUE: Output ONLY the exact spoken response. Never provide meta-advice ("Talk about your past...", "You should frame this..."). Write the exact, ready-to-speak first-person script.
2. CONTEXT INTEGRATION: Seamlessly incorporate your resume background, achievements, and target company context whenever relevant, without explicitly mentioning "according to my resume".
3. ADAPTIVE DOMAIN VERSATILITY: If technical or coding questions arise, answer them directly with expertise without breaking character.

RESPONSE STRUCTURE:
- Behavioral Questions: Use a crisp STAR format (Situation -> Task -> Action -> Result), highlighting measurable impact and metrics.
- Career / Background Questions: Provide a compelling 3-sentence summary of your trajectory and value alignment with the role.
- Weakness / Conflict Questions: State a real growth area, the proactive action taken, and the positive outcome.`,
  },
  {
    id: "lecture",
    label: "Lecture / Class",
    icon: "🎓",
    description: "Real-time explanation and answers during lectures",
    systemPrompt: `You are an expert tutor providing immediate, crystal-clear explanations and definitions during a live academic or technical lecture.

UNIVERSAL DIRECTIVES:
1. DIRECT DEFINITIONS: Provide clear, accurate definitions and explanations instantly without introductory filler.
2. HIGH SCANNABILITY: Use bold key terms and bullet points so complex concepts can be comprehended in seconds.
3. CONTEXTUAL REASONING: When questions appear in audio or slides, state the correct answer first, followed by the core underlying principle.`,
  },
  {
    id: "meeting",
    label: "Team Meeting",
    icon: "🤝",
    description: "Real-time participant updates for team meetings",
    systemPrompt: `You are an active participant in a high-level team meeting or standup. Provide direct status updates, technical decisions, and strategic feedback in the first person ("I" / "We").

UNIVERSAL DIRECTIVES:
1. READY-TO-SPEAK UPDATES: Output exact, professional updates and responses that can be read out loud to colleagues immediately.
2. ZERO FILLER: Omit meta-introductions and unnecessary preambles. Start directly with the status or decision.
3. ACTIONABLE SUMMARY: Structure updates as: 1) What was accomplished, 2) Current priority / next step, 3) Blockers or key trade-offs.`,
  },
  {
    id: "sales",
    label: "Sales",
    icon: "📈",
    description: "Real-time representative responses for sales calls",
    systemPrompt: `You are an elite sales representative on a live client call. Address prospect objections, state value propositions, and answer questions directly from the seller's perspective.

UNIVERSAL DIRECTIVES:
1. DIRECT SPOKEN SCRIPT: Output the exact, natural spoken response. Never use coaching notes ("Try reframing this..."). Write the actual words to say.
2. CONSULTATIVE & CONFIDENT: Frame responses around ROI, efficiency, and solving pain points.
3. BREVITY: Keep spoken lines punchy (1-2 sentences) to maintain fluid conversation.`,
  },
  {
    id: "recruiting",
    label: "Recruiting",
    icon: "🔍",
    description: "Real-time recruiter questions for interviews",
    systemPrompt: `You are a senior talent partner conducting a live interview assessment. Generate probing follow-up questions, evaluate candidate responses, and pitch the company vision.

UNIVERSAL DIRECTIVES:
1. DIRECT RECRUITER SCRIPT: Output the exact follow-up questions and pitches for the interviewer to read out loud.
2. TARGETED ASSESSMENTS: Focus questions on technical depth, cultural alignment, problem-solving, and leadership.
3. CONCISE PITCHES: State company value props and role expectations in brief, compelling bullet points.`,
  },
];

// ─── Profile Storage ──────────────────────────────────────────────────────────

export const PROFILE_STORAGE_KEY = "interview_profile_v1";

export interface InterviewProfile {
  mode: MeetingMode;
  resumeText: string;
  companyContext: string;
  customSystemPrompt: string;
  windowTitle: string;
  sttContextCache?: string;
}

export const DEFAULT_INTERVIEW_PROFILE: InterviewProfile = {
  mode: "general",
  resumeText: "",
  companyContext: "",
  customSystemPrompt: "",
  windowTitle: "",
  sttContextCache: "",
};

export const getInterviewProfile = (): InterviewProfile => {
  try {
    const stored = localStorage.getItem(PROFILE_STORAGE_KEY);
    if (!stored) return DEFAULT_INTERVIEW_PROFILE;
    const parsed = JSON.parse(stored);
    return {
      mode: parsed.mode || DEFAULT_INTERVIEW_PROFILE.mode,
      resumeText: parsed.resumeText || "",
      companyContext: parsed.companyContext || "",
      customSystemPrompt: parsed.customSystemPrompt || "",
      windowTitle: parsed.windowTitle || "",
      sttContextCache: parsed.sttContextCache || "",
    };
  } catch {
    return DEFAULT_INTERVIEW_PROFILE;
  }
};

export const saveInterviewProfile = (profile: InterviewProfile): void => {
  try {
    localStorage.setItem(PROFILE_STORAGE_KEY, JSON.stringify(profile));
  } catch (error) {
    console.error("Failed to save interview profile:", error);
  }
};

const MODE_PROMPTS_STORAGE_KEY = "meeting_modes_prompts_v1";

// Helper to get active definitions (loaded from storage if customized)
export const getMeetingModeDefinitions = (includeDeleted = false): MeetingModeDefinition[] => {
  try {
    const stored = localStorage.getItem(MODE_PROMPTS_STORAGE_KEY);
    if (!stored) {
      localStorage.setItem(MODE_PROMPTS_STORAGE_KEY, JSON.stringify(MEETING_MODE_DEFINITIONS));
      return MEETING_MODE_DEFINITIONS;
    }
    const parsed = JSON.parse(stored) as MeetingModeDefinition[];
    const merged = MEETING_MODE_DEFINITIONS.map(def => {
      const custom = parsed.find(p => p.id === def.id);
      return custom ? { ...def, ...custom } : def;
    });
    return includeDeleted ? merged : merged.filter(m => !m.deleted);
  } catch {
    return MEETING_MODE_DEFINITIONS;
  }
};

// Helper to save customized definitions
export const saveMeetingModeDefinitions = (definitions: MeetingModeDefinition[]): void => {
  try {
    localStorage.setItem(MODE_PROMPTS_STORAGE_KEY, JSON.stringify(definitions));
  } catch (error) {
    console.error("Failed to save meeting mode definitions:", error);
  }
};

// Helper to reset a specific mode to its default prompt
export const resetMeetingModeDefinition = (id: MeetingMode): MeetingModeDefinition[] => {
  const currentDefs = getMeetingModeDefinitions(true);
  const defaultDef = MEETING_MODE_DEFINITIONS.find(m => m.id === id);
  
  if (!defaultDef) return currentDefs;
  
  const updated = currentDefs.map(def => {
    if (def.id === id) {
      return { ...def, systemPrompt: defaultDef.systemPrompt, deleted: false };
    }
    return def;
  });
  
  saveMeetingModeDefinitions(updated);
  return updated;
};

// Helper to reset ALL modes to defaults
export const resetAllMeetingModeDefinitions = (): MeetingModeDefinition[] => {
  try {
    localStorage.removeItem(MODE_PROMPTS_STORAGE_KEY);
  } catch {}
  return MEETING_MODE_DEFINITIONS;
};

/**
 * Build a fully-enriched system prompt by combining:
 * 1. The mode's default prompt
 * 2. Optional custom override prompt
 * 3. Resume context (appended as background)
 * 4. Company/Job context (appended as background)
 */
export const buildEffectivePrompt = (
  profile: InterviewProfile,
  baseSystemPrompt?: string
): string => {
  const modeDefs = getMeetingModeDefinitions();
  const modeDef = modeDefs.find((m) => m.id === profile.mode);
  const modePrompt = modeDef?.systemPrompt || "";

  // Priority: profile custom override > selected system prompt card > mode prompt fallback
  const corePrompt =
    profile.customSystemPrompt?.trim() ||
    baseSystemPrompt?.trim() ||
    modePrompt ||
    "";

  const sections: string[] = [corePrompt];

  if (profile.resumeText?.trim()) {
    sections.push(
      `\n\n--- USER BACKGROUND (Resume / Bio) ---\n${profile.resumeText.trim()}\n--- END BACKGROUND ---`
    );
  }

  if (profile.companyContext?.trim()) {
    sections.push(
      `\n\n--- TARGET COMPANY / ROLE CONTEXT ---\n${profile.companyContext.trim()}\n--- END CONTEXT ---`
    );
  }

  return sections.join("");
};
