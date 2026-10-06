import importlib.util
import unittest
from pathlib import Path

spec = importlib.util.spec_from_file_location("quality", Path(__file__).with_name("retrieval-quality.py"))
quality = importlib.util.module_from_spec(spec)
spec.loader.exec_module(quality)

class QualityTests(unittest.TestCase):
    def fixture(self):
        return {"identity": dict(model="m", revision="r", task="prepared", tokenizer="t", corpus="c"), "documents": [{"id": "a", "vector": [1.,0.]}, {"id": "b", "vector": [0.,1.]}], "queries": [{"id": "q", "vector": [1.,0.], "expected": ["a"]}]}
    def test_equal_vectors_pass_and_changed_rankings_are_visible(self):
        baseline, candidate = self.fixture(), self.fixture()
        self.assertEqual(quality.compare(baseline, candidate, 1)["candidate_recall"], 1.)
        candidate["documents"][0]["vector"] = [0.,1.]
        candidate["documents"][1]["vector"] = [1.,0.]
        result = quality.compare(baseline, candidate, 1)
        self.assertEqual(result["candidate_recall"], 0.)
        self.assertEqual(result["top_k_overlap"], 0.)
    def test_model_mismatch_and_invalid_vectors_are_rejected(self):
        baseline, candidate = self.fixture(), self.fixture()
        candidate["identity"]["revision"] = "other"
        with self.assertRaises(ValueError): quality.compare(baseline, candidate, 1)
        candidate = self.fixture()
        candidate["documents"][0]["vector"] = [float("nan"),0.]
        with self.assertRaises(ValueError): quality.compare(baseline, candidate, 1)

if __name__ == "__main__": unittest.main()
