# Phase 9 Plan: Semantic Memory Client

## 1. Objective

The goal of Phase 9 is to implement the **Semantic Memory Client** inside the `memory` crate. This allows PRISM to store and retrieve historical repair profiles using vector embeddings, helping the AI engine match current problems against past resolutions.

We will:
- Implement a vector similarity search engine in the Rust `memory` crate.
- Configure a client to fetch vector embeddings from the local Ollama service (`/api/embeddings` endpoint) for any system issue description.
- Implement a local vector store (`prism_memory.json`) that saves text strings alongside their high-dimensional floating-point vectors.
- Implement Cosine Similarity math in Rust to find the top $K$ most similar historical events.
- Update the client and CLI to index repair outcomes upon successful completion.

---

## 2. Similarity Search Architecture

```mermaid
graph TD
    Query[User Query: 'My computer is slow'] --> Embed[Ollama Embeddings API]
    Embed -->|Vector [f32]| Search[Cosine Similarity Engine]
    Store[(prism_memory.json)] -->|All Vectors| Search
    Search -->|Top-K Matches| Reasoning[AI Reasoning Context]
```

The Cosine Similarity formula used to score matches is:

$$\text{Similarity}(A, B) = \frac{\sum_{i=1}^n A_i B_i}{\sqrt{\sum_{i=1}^n A_i^2} \sqrt{\sum_{i=1}^n B_i^2}}$$

---

## 3. Integration Points

1. **Insert**: When a repair completes successfully (or fails), the orchestrator gets the embedding for the user's `raw_input` and saves the event details + vector to `prism_memory.json`.
2. **Search**: Before executing a repair plan, the orchestrator searches for similar past inputs. If a similar past input resulted in a failure with the same corrective actions, the engine can adjust the plan or prompt the user.
